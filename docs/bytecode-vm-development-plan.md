# GoScript 字节码虚拟机开发计划（契约驱动全量交付版）

> **文档定位**：GTS_R 执行管线从「AST 树遍历」整体迁移到「字节码 + 栈式 VM」的**全量交付开发计划**。
>
> **核心需求**（最高约束，覆盖一切其他考量）：
> > **VM 必须能承接当前树遍历解释器的所有功能。没有任何"先支持 X、后支持 Y"的灰度空间。**
>
> 因此本计划的性质是**契约驱动全量交付（contract-driven full replacement）**，不是增量探索。
> 验收标准从"双跑对齐"升级为"**VM 单跑全绿**"——所有现有 fixture 必须在 VM 下独立通过，不依赖树遍历兜底。
>
> **编写日期**：2026-06-22
> **基线代码状态**：执行器为树遍历（`src/evaluator/`）；`src/object/vm.rs:34` 的 `VirtualMachine` 是运行时壳子，非字节码 VM。
> **功能契约基线**：51 个 parity fixture（`tests/fixtures/parity/`）+ 12 个 verification 套件（`gts/verification/`）。

---

## 0. 阅读约定

- **MUST / SHOULD / MAY** 沿用 RFC 2119 语义。
- 所有 `文件:行号` 引用以本文档编写日的代码为准；改动后行号会漂移，**以符号名为最终依据**。
- **「契约门」** = VM 单跑该 fixture / 该 AST 节点 / 该方法，与现有树遍历输出（stdout / exit code / error message）逐字节一致。
- **「禁改清单」** = 在 VM 交付期内禁止破坏性修改的现有文件（见 §5.2），违反即视为破坏契约。
- 「全量交付」= §2 契约矩阵中**每一格**都打勾，不得留任何 TODO / placeholder。

---

## 1. 需求重述与改造动机

### 1.1 需求原文

> **「VM 能够承接之前所有的功能」**

拆解为三条可执行的硬约束：

| 编号 | 约束 | 验收形式 |
|------|------|----------|
| **REQ-1** | VM 单跑现有 51 个 parity fixture 全绿，输出与树遍历逐字节一致 | `cargo test --test bytecode_parity` 全绿，逐 fixture 比对 |
| **REQ-2** | VM 单跑现有 12 个 verification 套件全绿 | `gts\verification\**` 在 `--exec-mode=bytecode` 下通过 |
| **REQ-3** | VM 覆盖 AST 中**全部** 18 种 Stmt + 27 种 Expr + Match 模式系统 + 类系统 + 模块系统 + 类型注解，无任何遗漏节点 | §3 编译器对照表 100% 覆盖；每个 AST 变体至少 1 个 fixture 触发 |

**REQ-1/2 是黑盒验收，REQ-3 是白盒覆盖度。三者必须同时满足。**

### 1.2 现状诊断（经代码核实）

当前执行链路：`Lexer → Parser → AST → eval_program/eval_expr/eval_stmt（递归 match）`。

| # | 痛点 | 代码位置 |
|---|------|----------|
| P1 | AST 重复遍历、无编译缓存 | `evaluator/expressions.rs:16` 巨型 match |
| P2 | 控制流靠抛异常 | `eval_core.rs:11-12,67-83`，`BREAK_SIGNAL="__break__"` |
| P3 | `return` 靠 `Object::Return(Box<Object>)` | `value.rs:37` |
| P4 | 变量查找是字符串哈希 + 父链遍历 | `environment.rs:62` `get()` |
| P5 | 每个 AST 节点都 `check_timeout` | `eval_core.rs:50`、`expressions.rs:17` |
| P6 | `Box<dyn Fn>` 回调驱动求值 | `vm.rs:18` `EvaluatorFn` |

VM 的价值正是对症消除 P1–P4，顺带缓解 P5/P6。

### 1.3 明确排除（非目标）

- ❌ **不**做灰度共存到永远——树遍历最终下线（但保留至 §6 阶段验收全绿后才移除）。
- ❌ **不**改 `Object` 枚举内存布局（不上 NaN-boxing、不换值表示）。
- ❌ **不**重写 Object 系统、异步运行时、stdlib——全部复用。
- ❌ **不**上寄存器式 VM。栈式编译器与 `eval_expr` 一一对应，是全量覆盖最快的路径。

---

## 2. 功能契约矩阵（全量覆盖度清单）

> 这是本计划的**核心**。VM 必须让下表每一格都打勾。"参照实现"列指向树遍历中该功能的实现位置，作为 VM 的语义蓝本。

### 2.1 语句（Stmt）— 18 种

| AST 节点 | 契约 fixture | 参照实现 |
|----------|--------------|----------|
| `Let` | `01_variables`、`basic_expression` | `eval_core.rs` `eval_let` |
| `Const` | `01_variables`、`export_const` | `eval_const` |
| `Var` | `01_variables` | `eval_var` |
| `FuncDecl` | `04_functions`、`function_call`、`recursive_function` | `eval_func_decl` |
| `ClassDecl` | `08_classes`、`class_*`（8 个） | `eval_class_decl` |
| `Block` | `03_control_flow` | `eval_block` |
| `If` | `03_control_flow`、`control_flow` | `eval_if` |
| `While` | `while_continue` | `eval_while` |
| `For` | `for_break`、`nested_loops`、`loop_array_build` | `eval_for` |
| `ForIn` | （需补 fixture） | `eval_for_in` |
| `ForOf` | （需补 fixture） | `eval_for_of` |
| `Return` | `04_functions` | `eval_return` |
| `Break` | `for_break` | sentinel `BREAK_SIGNAL` → VM 改为跳转 |
| `Continue` | `while_continue` | sentinel `CONTINUE_SIGNAL` → VM 改为跳转 |
| `Throw` | `throw_catch_*`、`try_catch` | `eval_throw` |
| `Try` | `try_catch`、`try_finally_no_throw`、`catch_finally_order` | `eval_try` |
| `Expr` | 全部 fixture | `expressions.rs:77` |
| `Labeled` | （需补 fixture） | `eval_stmt` 透传 |
| `Import` | `relative_require`、`nested_relative_require`、`project_module_require`、`directory_module_index`、`module_cache`、`module_exports_object`、`import_default_like` | `eval_import` |
| `Export` | `export_const`、`export_function_alias` | `eval_export` |

### 2.2 表达式（Expr）— 27 种

| AST 节点 | 契约 fixture | 参照实现 |
|----------|--------------|----------|
| `Ident` | 全部 | `eval_ident` |
| `Number` / `String` / `Bool` / `Null` / `Undefined` | `basic_expression` | `eval_expr` 直接构造 |
| `Template` | `template_literals` | `string_lit.rs` `eval_template` |
| `Regexp` | （需补 fixture） | `string_lit.rs` `eval_regexp_lit` |
| `This` | `class_method_this` | `environment.rs:26` |
| `Super` | `class_super_method_override`、`class_implicit_super`、`class_inheritance_*` | `eval_super` |
| `Array` | `arrays_objects`、`array_*`（6 个） | `eval_array` |
| `Object` | `arrays_objects`、`object_*`（4 个） | `eval_object_lit` |
| `Prefix` | `02_operators` | `eval_prefix` |
| `Infix` | `02_operators`、`comparison_edges`、`truthy_logic` | `eval_infix` |
| `Ternary` | `truthy_logic` | `eval_ternary` |
| `Assign` | `class_field_update`、`array_index_assignment` | `eval_assign` |
| `Call` | `function_call`、`object_method_call` | `eval_call` |
| `Member` | `object_nested_access`、`object_method_call` | `eval_member` |
| `Index` | `array_index_assignment`、`object_computed_key` | `eval_index` |
| `Optional` | （需补 fixture，`?.` 可选链） | `eval_optional` |
| `Func`（函数表达式） | `function_closure` | `eval_func_expr` |
| `Arrow`（箭头函数） | `function_closure`、`array_*`（map/filter 回调） | `eval_arrow` |
| `New` | `class_*` 系列 | `eval_new` |
| `Await` | `12_async_await`、`11_promises` | `eval_await` |
| `Spread` | （需补 fixture） | `eval_expr` 透传 |
| `Match` | `match_*`（7 个） | `match_eval.rs` `eval_match` |
| `Class`（类表达式） | （需补 fixture） | `build_class` |

### 2.3 Match 模式系统（Pattern）— 5 种

| Pattern | 契约 fixture | 参照 |
|---------|--------------|------|
| `Literal` | `match_basic`、`match_string` | `match_eval.rs` |
| `Ident` | `match_null`、`match_no_arm_catch` | |
| `Wildcard` | `match_default_only` | |
| `Or` | （需补 fixture） | |
| `Range` | （需补 fixture） | |

> `MatchBody::{Expr, Block}` 与 `MatchArm.guard` 也 MUST 覆盖（`match_block_body`）。

### 2.4 类系统（ClassMember）

| 成员 | 契约 fixture |
|------|--------------|
| `Method`（实例/静态） | `class_basic`、`class_method_this` |
| `Field`（实例/静态） | `class_field_update` |
| `Constructor` | `class_inheritance_constructor`、`class_implicit_super` |
| `super_` 继承链 | `class_inheritance_method`、`class_super_method_override` |
| `is_async` 方法 | `12_async_await`（async 方法内 await） |

### 2.5 模块系统

| 能力 | 契约 fixture |
|------|--------------|
| 相对路径 `require` | `relative_require`、`nested_relative_require` |
| `project.toml` 入口 | `project_module_require` |
| 目录模块 `index` | `directory_module_index` |
| 模块缓存（防重入） | `module_cache` |
| `module.exports = {}` | `module_exports_object` |
| `import default` 语义 | `import_default_like` |
| `export { }` / `export const` / 别名 | `export_const`、`export_function_alias` |

### 2.6 类型注解 + 内建对象 + 标准库（必须复用，不重写）

- **类型注解**：`Param.type_anno` / `Field.type_anno` / `return_t` 解析 MUST 通过；运行期检查对齐树遍历行为（`type_check` 开关，`vm.rs:38`）。
- **内建全局**：`Math / JSON / console / Object / Array / String / Error / RegExp / Date / Map / Set / Promise / Symbol`（由 `register_globals` 注入）—— VM 直接复用同一套 `Object::Builtin`，**不重新实现**。
- **22 个数组方法 + 19 个字符串方法 + Promise.all/race/resolve/reject**：复用现有 `builtins.rs` / `methods.rs` 实现。
- **32 个 `@std/*` 模块**：通过现有 `load_native_module` 接入，VM 不感知。

> **结论**：内建/标准库这条线 VM **零实现成本**——全部通过 `apply_function` 复用。VM 只需正确调用，不需重新实现任何方法。

---

## 3. 目标架构

### 3.1 拓扑

```
            ┌───────────────────┐
AST ──────▶ │  Compiler (新增)   │ ──▶ Chunk (常量池 + 字节码 + 行号表 + 保护域表)
            └───────────────────┘            │
                                              ▼
                                    ┌───────────────────┐
                                    │ BytecodeInterp    │ ──▶ Object
                                    │ (新增, 栈式 VM)    │
                                    └───────────────────┘
                                              │
                  复用 Object / Promise / Awaitable / EventLoop / Builtins / Stdlib
```

### 3.2 新增模块

```
src/bytecode/
├── mod.rs          // 入口：compile / interpret
├── opcode.rs       // Opcode 枚举 + 反汇编
├── chunk.rs        // Chunk + ProtectedRegion
├── compiler.rs     // AST → Chunk（对照表见 §3.5，覆盖全部节点）
├── frame.rs        // CallFrame：ip / proto / slots / upvalues / this
├── closure.rs      // ClosureData / FunctionProto / Upvalue（Lua 式）
├── interp.rs       // 主 dispatch loop
├── awaitable.rs    // BytecodeFrameAwaitable（实现 Awaitable，支持 OpAwait 挂起）
└── native_bridge.rs// VM ↔ 现有 builtins/methods 的桥接（apply_function 的字节码分支）
```

> **命名避坑**：不复用 `VirtualMachine`（已被 `vm.rs:34` 占用，语义是运行时壳子）。VM 解释器命名为 `BytecodeInterp`。

### 3.3 Object 兼容策略

- **新增 1 个变体** `Object::Closure(Rc<ClosureData>)`。`Function` 变体保留（下线前共存）。
- **不**新增其他值类型。`Object::Return` 在 VM 路径下**不产生**（return 走 `OpReturn`），但变体保留。
- 现有所有 `Builtin` / `Class` / `Instance` / `Promise` / `Map` / `Set` / `Date` / `Regexp` **零改动**。

### 3.4 修改面（最小化）

| 文件 | 改动 | 破坏树遍历？ |
|------|------|:---:|
| `ast/mod.rs` | **不改**（AST 是编译边界） | 否 |
| `object/value.rs` | 新增 `Object::Closure` 变体 | 否（纯新增） |
| `object/vm.rs` | 加 `exec_mode` + `compile_cache` | 否 |
| `object/environment.rs` | **不改** get/set；VM 只用 root 的 globals/this/module_dir | 否 |
| `evaluator/expressions.rs:745` `apply_function` | 新增 `Object::Closure` 臂，委托 VM 执行 | 否（新增 match 臂） |
| `evaluator/methods.rs` 同名调用点 | 同上 | 否 |
| `runtime/mod.rs` `Session::run` | 入口分流 `exec_mode` | 否 |
| `eval_core.rs` sentinel | 保留至下线 | 否 |

### 3.5 编译器 AST → Opcode 对照表（全节点覆盖，VM 交付的覆盖度证据）

> 编译器实现时逐行核对，确保下表**每一格都有对应编译分支**。这是 REQ-3 的直接交付物。

**字面量与标识符**
- `Number/Bool` → `OpConst`
- `String/Template/Regexp` → `OpConst`（编译期求值）或 template 拼接序列
- `Null/Undefined` → `OpConst`（哨兵）
- `Ident` → 变量解析 pass 后 → `OpLoadLocal / OpLoadUpvalue / OpLoadGlobal`
- `This` → `OpLoadThis`
- `Super` → `OpSuperMethod(name)`

**运算符**
- 前缀 `!/-/+` → `OpNot / OpNeg / OpIdentity`
- 中缀算术 → `OpAdd/Sub/Mul/Div/Mod/Pow`（字符串 `+` 特化为 `OpConcat`）
- 中缀比较 → `OpEq/Neq/Lt/Le/Gt/Ge`
- 中缀逻辑 → 短路：`&&`=`OpJumpIfFalse`，`||`=`OpJumpIfTrue`

**控制流**
- `If` → `cond; OpJumpIfFalse(L_else); then; OpJump(L_end); L_else: else; L_end:`
- `While` → `L_start: cond; OpJumpIfFalse(L_end); body; OpJump(L_start); L_end:`
- `For` → init; L_start: cond; OpJumpIfFalse(L_end); body; post; OpJump(L_start)
- `Break/Continue` → 回填到循环栈的 `break_target / continue_target`
- `Labeled break/continue` → 循环栈记录 label，匹配后回填

**函数与闭包**
- `FuncDecl / Func / Arrow` → 编译为 `FunctionProto`，声明处生成 `OpClosure(idx)` 运行期绑定 upvalue
- `Arrow(expr body)` → 隐式 `OpReturn`
- `Call` → 参数求值压栈 + `OpCall(arg_count)`
- 参数默认值 → `OpLoadArg(i); OpJumpIfNotNull(skip); <default>; skip:`
- 参数 spread → `OpSpread` 收集到数组

**对象模型**
- `Array` → 各元素求值压栈（spread 用 `OpSpread`）+ `OpNewArray(n)`
- `Object` → `OpNewObject` + 逐属性 `OpSetProperty(name)`（computed key 走 `OpSetIndex`）
- `Member` → `OpGetProperty(name)`
- `Index` → `OpGetIndex`
- `Optional` → 求值对象 + `OpJumpIfNull(skip)`（跳过后续 member/call）
- `Assign` → 目标是 `Ident/Member/Index` 三种，分别 `OpStoreLocal/Global/Upvalue`、`OpSetProperty`、`OpSetIndex`
- 复合赋值 `+=` 等 → 读 + 算 + 写（在编译器展开）
- `New` → 参数压栈 + `OpNew(name)`
- `Spread` → `OpSpread`

**Match**
- `Match` → 对 scrutinee 求值后，逐 arm 生成「模式测试 + 命中跳转」。模式编译：
  - `Literal` → `OpEq` 比较
  - `Ident` → 绑定到局部槽（总是命中）
  - `Wildcard` → 无条件命中
  - `Or` → 多个测试，任一命中即跳
  - `Range` → `OpGe(start); OpJumpIfFalse; OpLe(end)`
  - `guard` → 命中后 `OpJumpIfFalse(next_arm)`
  - `MatchBody::{Expr,Block}` → 求值后 `OpJump(end)`

**类**
- `Class` → 编译为：构造类对象（`OpNewClass`）、每个方法体单独编译为 `FunctionProto`、`OpDefineMethod(name, is_static)`、处理 `super_` 链
- `Constructor` → 调用 `super` 后执行字段初始化

**错误处理**
- `Throw` → `OpThrow`
- `Try/Catch/Finally` → 在 `Chunk.protected_regions` 写入区间；handler_ip 指向 catch/finally

**模块**
- `Import` → 调用现有 `importer` 回调（`vm.rs:30` `ImporterFn`），把导出值绑定到本地槽
- `Export` → 求值后写入模块导出表

**类型注解**
- 全部忽略运行期语义（对齐树遍历 `type_check` 默认 false）；若 `type_check=true` 则在赋值/返回处插入运行期检查调用

**异步**
- `async function/method` → `FunctionProto.is_async = true`
- `Await` → 求 operand + `OpAwait`

> 编译器 PR 合并前 MUST 提交一份"AST 节点 → 已实现编译分支"的覆盖度核对表（markdown 或测试生成），任何未覆盖节点标注为**阻断**而非 TODO。

---

## 4. 阶段计划（契约门驱动）

> 与旧版"灰度探索"不同：每个阶段的验收门是**该阶段覆盖的 fixture 在 VM 下单跑全绿**，不是"双跑对齐"。
> 双跑对齐是**开发期**的工具（用树遍历输出做 oracle），但**阶段验收**要求 VM 单跑本身全绿。

### 阶段 0：脚手架与可观测性

**目标**：建立完整骨架，跑通 `1 + 2`，建立反汇编与指令计数。

**交付**：
- `bytecode/{opcode,chunk,compiler,interp,mod}.rs` 骨架
- `Opcode::{Const, Add, Pop, Return}` 实现
- `bytecode/mod.rs`：`pub fn compile(&Program) -> Chunk`、`pub fn interpret(&Chunk, &EnvRef) -> Object`
- `object/vm.rs`：加 `pub exec_mode: AtomicU8`（0=TreeWalk 默认，1=Bytecode）
- 反汇编函数 `Chunk::disassemble()`

**契约门**：
- [ ] `cargo build` 通过，`cargo clippy --lib` 0 error
- [ ] 单测：`interpret(compile(parse("1 + 2")))` == `Object::Number(3.0)`
- [ ] 树遍历行为不变（现有全部测试全绿）

---

### 阶段 1：表达式全集 + 变量（契约：表达式层全量）

**目标**：覆盖 §3.5 字面量/运算符/变量存取**全部**分支。

**交付**：
- 全部算术/比较/逻辑/一元/字符串拼接指令
- `Let/Const/Var` 声明 → 全局名字表
- `Template/Regexp` 字面量编译

**契约门（VM 单跑）**：
- [ ] `basic_expression` 全绿
- [ ] `01_variables` 全绿
- [ ] `02_operators` 全绿
- [ ] `comparison_edges` 全绿
- [ ] `truthy_logic` 全绿
- [ ] `template_literals` 全绿
- [ ] **覆盖度核对**：§3.5 字面量/运算符/Ident 三节全部打勾

---

### 阶段 2：控制流全集（契约：控制流全量，含跳转替代异常）

**目标**：if/while/for/for-in/for-of/break/continue/labeled 全量，**用跳转替代 sentinel 异常**。

**交付**：
- `OpJump/JumpIfFalse/JumpIfTrue/Loop`
- 编译器循环栈（含 label）
- 迭代器协议（for-in 遍历 key、for-of 遍历 value；数组/字符串/Map/Set）

**契约门（VM 单跑）**：
- [ ] `control_flow`、`03_control_flow` 全绿
- [ ] `while_continue` 全绿
- [ ] `for_break` 全绿
- [ ] `nested_loops` 全绿
- [ ] `loop_array_build` 全绿
- [ ] **里程碑证据**：`for(i=0;i<1_000_000;i++){}` 在 VM 下堆分配次数显著低于树遍历（树遍历每轮 2 个 `ErrorData`，VM 应为 0）——这是 VM 价值的质心证据
- [ ] **补 fixture**：`for_in_object`、`for_of_array`、`labeled_break` 三例补进 parity 集（现有缺）

---

### 阶段 3：函数 + CallFrame + native 互调（契约：函数调用全量）

**目标**：用户函数全量，建立 VM ↔ native 双向互调。

**交付**：
- `closure.rs`：`FunctionProto` / `ClosureData`
- `frame.rs`：`CallFrame { ip, proto, slots, upvalues, this }`
- `OpClosure/Call/Return/ReturnNull`
- **关键改动**：`expressions.rs:745` `apply_function` 与 `methods.rs` 调用点新增 `Object::Closure` 臂，委托 `bytecode::interp::call_closure`
- 调用约定：callee 在栈顶，参数紧贴其下，`OpCall(n)` 弹参数+callee，建新帧；`OpReturn` 弹帧、压返回值
- 参数默认值、spread、`arguments` 对象（若现有支持）

**契约门（VM 单跑）**：
- [ ] `04_functions`、`function_call`、`recursive_function` 全绿
- [ ] **native 调 VM 闭包**：`[1,2,3].map(x=>x*2)` 在 VM 下全绿（这验证 `apply_function` 双向桥）
- [ ] `string_methods` 全绿（验证 native 方法回调）
- [ ] **覆盖度核对**：`Func/Arrow/Call/FuncDecl` 全部打勾

---

### 阶段 4：闭包与 Upvalue（契约：词法捕获全量）

**目标**：正确实现 Lua 式 upvalue，覆盖所有闭包语义。

**模型**：
- `Upvalue`：开放（指向外层帧栈槽）/ 闭合（迁移到 `Rc<RefCell<Object>>` 堆盒）
- 编译期变量解析 pass：局部槽 / upvalue / 转发 upvalue / 全局 四态
- Interp 维护 `open_upvalues: BTreeMap<slot_idx, Vec<Rc<Upvalue>>>`，帧退出时闭合

**交付**：
- `OpLoadUpvalue/StoreUpvalue`
- 编译器变量解析 pass

**契约门（VM 单跑）**：
- [ ] `05_closures`、`function_closure` 全绿
- [ ] **经典语义专项（必测，现有缺则补 fixture）**：
  - 循环内多闭包捕获循环变量
  - 闭包修改捕获变量对外可见（counter 模式）
  - 返回闭包后原帧退出（验证 upvalue 闭合）
  - IIFE 捕获
- [ ] debug / release 双跑结果一致（验证无悬空栈槽 UB）

---

### 阶段 5：对象模型全集（契约：Array/Hash/Class/Super 全量）

**目标**：覆盖 §2.2/2.4 全部对象语义。

**交付**：
- `OpNewArray/NewObject/GetProperty/SetProperty/GetIndex/SetIndex/Spread`
- `OpNew/DefineMethod/NewClass`
- `CallFrame.this` 绑定（对齐 `environment.rs:26-29`）
- super 方法解析（复用 `methods.rs` 逻辑）
- `build_class` 逻辑下沉到编译器 + 运行期类构造

**契约门（VM 单跑）**：
- [ ] `arrays_objects`、`06_arrays`、`07_objects`、`08_classes` 全绿
- [ ] `array_*`（6 个）、`object_*`（4 个）、`class_*`（8 个）全部全绿
- [ ] **专项**：`class_super_method_override`、`class_inheritance_constructor`、`class_implicit_super`、`class_method_this`、`class_field_update` 全绿
- [ ] `object_computed_key`、`object_nested_access`、`object_method_call` 全绿
- [ ] **覆盖度核对**：`Array/Object/Member/Index/New/Super/This/Assign/Spread/Match/Class` 全部打勾

---

### 阶段 6：错误处理全集（契约：try/catch/finally 全量）

**目标**：protected-region 表替代异常控制流。

**交付**：
- `OpThrow`
- `Chunk.protected_regions`
- Interp 抛错 unwind：沿帧栈查 region，命中跳 handler，否则向上
- finally 语义：无论是否抛错都执行；finally 内 throw 覆盖原异常（对齐 JS）

**契约门（VM 单跑）**：
- [ ] `09_errors` 全绿
- [ ] `try_catch`、`try_finally_no_throw`、`catch_finally_order` 全绿
- [ ] `throw_catch_string`、`throw_catch_error` 全绿
- [ ] `match_no_arm_catch`（match 无 arm 抛错）全绿
- [ ] **错误位置**：`Chunk.lines` 反查 `Position`，error message 与树遍历**逐字符一致**
- [ ] **覆盖度核对**：`Throw/Try` + Match `Ident`（无 arm 捕获）打勾

---

### 阶段 7：Match 全集 + 类型注解（契约：模式匹配与类型全量）

**目标**：覆盖 §2.3 全部 Pattern 变体 + 类型注解运行期检查。

**交付**：
- Match 编译（§3.5 Match 节）
- 类型注解：`type_check=true` 时插入运行期检查调用

**契约门（VM 单跑）**：
- [ ] `match_basic`、`match_string`、`match_null`、`match_boolean`、`match_default_only`、`match_block_body`、`match_no_arm_catch` 全绿
- [ ] **补 fixture**：`match_or`、`match_range`、`match_guard`（现有缺）
- [ ] `10_typeof` 全绿
- [ ] **覆盖度核对**：`Match` + 5 种 Pattern + `MatchBody` + `guard` 全部打勾

---

### 阶段 8：模块系统全集（契约：模块全量）

**目标**：覆盖 §2.5 全部模块能力。

**交付**：
- `Import/Export` 编译：调用现有 `importer` 回调（`vm.rs:30`），绑定到本地槽
- 循环依赖检测（复用现有 module cache）
- re-export `export { } from "..."`

**契约门（VM 单跑）**：
- [ ] `relative_require`、`nested_relative_require`、`project_module_require`、`directory_module_index`、`module_cache`、`module_exports_object`、`import_default_like`、`export_const`、`export_function_alias` 全绿
- [ ] **覆盖度核对**：`Import/Export` 全部打勾

---

### 阶段 9：异步全集（契约：Promise/async-await 全量）

**目标**：覆盖 §2.6 异步语义，复用现有 Awaitable/EventLoop。

**交付**：
- `OpAwait`
- `bytecode/awaitable.rs`：`BytecodeFrameAwaitable` 实现 `Awaitable`
- async 函数：`FunctionProto.is_async=true`
- 接线 `async_runtime/awaitable_bridge.rs`、`object/event_loop.rs`

**契约门（VM 单跑）**：
- [ ] `11_promises`、`12_async_await` 全绿
- [ ] Promise.all/race/resolve/reject 输出时序与树遍历一致
- [ ] async 函数内 try/catch 捕获 await 抛错一致
- [ ] setTimeout/setInterval 行为一致（复用 TimerWheel）
- [ ] **覆盖度核对**：`Await` + async `FuncDecl/Method/Arrow` 打勾

---

### 阶段 10：全量验收 + 默认切换 + 树遍历下线评估

**目标**：全部契约门通过后，默认执行器切 VM；评估是否移除树遍历。

**契约门（终验）**：
- [ ] **REQ-1**：`cargo test --test bytecode_parity` —— 51 个 fixture 在 VM 下**单跑**全绿（不再依赖树遍历）
- [ ] **REQ-2**：`gts\verification\**` 12 套件在 `--exec-mode=bytecode` 下全绿
- [ ] **REQ-3**：§3.5 编译器覆盖度表 100%，无未覆盖 AST 节点
- [ ] **补全所有"需补 fixture"**（§4 各阶段标注的 for-in/for-of/labeled/match-or/match-range/regexp/optional/spread/class-expr）
- [ ] 性能基准：`bench/scripts/bench_server.gs` 在 fib / 字符串拼接 / Promise 创建三类场景下 VM 不劣于树遍历
- [ ] `Session::new()` 默认 `ExecMode::Bytecode`；保留 `--exec-mode=tree` flag
- [ ] **决定**：树遍历保留为 legacy fallback 还是移除（本阶段只决定，移除动作另立 PR）

---

## 5. 工程纪律（契约保护机制）

### 5.1 分支与提交

- 分支 `feat/bytecode-vm`，**独立于其他功能线**。
- 每阶段一个 PR，标题 `[bytecode-N]`，N 为阶段号。
- 单 commit 单一职责；禁止"顺手重构"。

### 5.2 禁改清单（VM 交付期内）

以下文件**严禁破坏性修改**，只允许新增：

- `src/object/value.rs` 现有 `Object` 变体（仅允许新增 `Closure`）
- `src/object/environment.rs` 的 `get/set` 逻辑
- `src/object/promise.rs`、`src/object/awaitable.rs`、`src/object/event_loop.rs`、`src/object/timer_wheel.rs`
- `src/evaluator/*` 现有函数（`apply_function` 仅允许新增 match 臂）
- `src/stdlib/*`（全部 32 个 `@std/*` 与 helpers）
- `src/ast/*`、`src/lexer/*`、`src/parser/*`（前端零改动，AST 是编译边界）

违反需在 PR 描述说明"为何无法用新增替代"，并需评审通过。

### 5.3 契约门不可跳过

- 每个阶段的契约门是**硬阻断**。任一 fixture 在 VM 单跑下失败，**禁止合并、禁止进入下一阶段**。
- 开发期可用树遍历输出做 oracle 比对（双跑对齐作为开发工具），但**阶段验收要求 VM 单跑本身全绿**。
- 每阶段 PR MUST 附带：① 该阶段契约 fixture 的 VM 单跑结果 ② §3.5 覆盖度表增量。

### 5.4 编译缓存与超时

- `VirtualMachine` 加 `compile_cache: RefCell<HashMap<SourceHash, Rc<Chunk>>>`，函数体只编一次。
- 编译是纯函数（除错误报告外无副作用）。
- Interp 主循环 `instr_since_check` 计数，每 1024 条指令检查一次 deadline（消除 P5）。**禁止逐指令检查**。

### 5.5 缺失 fixture 的处置

本计划标注了若干"需补 fixture"（for-in、for-of、labeled、match-or、match-range、regexp、optional-chain、spread、class-expr）。这些 MUST 在对应阶段补进 `tests/fixtures/parity/`，并**先在树遍历下验证为绿**（确认是正确语义），再用作 VM 的契约 oracle。

---

## 6. 风险登记册

| ID | 风险 | 影响 | 对策 | 阶段 |
|----|------|------|------|------|
| R1 | 闭包捕获语义（循环变量、arguments） | 高 | 阶段 4 专项 fixture；先保守"捕获即快照"再优化 | 4 |
| R2 | 错误位置/堆栈信息丢失 | 中 | `Chunk.lines` 每指令记 `Position`；阶段 6 逐字符比对 message | 6 |
| R3 | native ↔ VM 闭包互调死循环 | 中 | `apply_function` 单一入口；阶段 3 即建立并测试 | 3 |
| R4 | finally 覆盖异常语义 | 中 | protected-region + `catch_finally_order` fixture | 6 |
| R5 | async 续体跨帧挂起/恢复 | 高 | `BytecodeFrameAwaitable` 完整序列化帧；先单 await 再多 await | 9 |
| R6 | Match 模式编译遗漏变体 | 中 | §3.5 Match 节逐变体编译；阶段 7 覆盖度核对 | 7 |
| R7 | 模块循环依赖 / 缓存失效 | 中 | 复用现有 module cache；阶段 8 `module_cache` fixture | 8 |
| R8 | 过早优化（NaN-boxing / 寄存器化） | 中 | 本计划明确排除；优化等阶段 10 后另立项 | 全程 |
| R9 | "需补 fixture"本身语义错误 | 中 | 补的 fixture 先在树遍历下验证为绿，再做 VM oracle | 各阶段 |

---

## 7. 验收总表

| 阶段 | 契约 fixture（VM 单跑全绿） | 覆盖度增量 |
|------|----------------------------|-----------|
| 0 | （骨架 `1+2`） | — |
| 1 | `basic_expression` `01_variables` `02_operators` `comparison_edges` `truthy_logic` `template_literals` | 字面量/运算符/Ident |
| 2 | `control_flow` `03_control_flow` `while_continue` `for_break` `nested_loops` `loop_array_build` + 补 for-in/for-of/labeled | 控制流全集 |
| 3 | `04_functions` `function_call` `recursive_function` `string_methods` + native↔VM 互调 | Func/Arrow/Call |
| 4 | `05_closures` `function_closure` + 补闭包专项 | Upvalue |
| 5 | `arrays_objects` `06~08_basics` `array_*`(6) `object_*`(4) `class_*`(8) | 对象模型全集 |
| 6 | `09_errors` `try_*`(3) `throw_catch_*`(2) `match_no_arm_catch` | Throw/Try |
| 7 | `match_*`(7) `10_typeof` + 补 match-or/range/guard | Match 全集 |
| 8 | `relative_require` `nested_relative_require` `project_module_require` `directory_module_index` `module_cache` `module_exports_object` `import_default_like` `export_const` `export_function_alias` | 模块全集 |
| 9 | `11_promises` `12_async_await` | Await/async |
| 10 | **全部 51+ fixture VM 单跑全绿** | **REQ-1/2/3 终验** |

---

## 8. 关键接口速查（编写日核实）

| 符号 | 位置 | 用途 |
|------|------|------|
| `eval_expr` | `evaluator/expressions.rs:16` | 树遍历入口，Compiler 的语义蓝本 |
| `apply_function` | `evaluator/expressions.rs:745` | 统一调用入口，阶段 3 加 `Object::Closure` 臂 |
| `control_signal` | `evaluator/eval_core.rs:599` | sentinel 识别（VM 不再用） |
| `BREAK/CONTINUE_SIGNAL` | `eval_core.rs:11-12` | sentinel 字符串（保留至下线） |
| `Object::Return` | `value.rs:37` | return 盒子（VM 不产生） |
| `Object` 枚举 | `value.rs:24` | 值类型，新增 `Closure` |
| `Function` / `Builtin` / `CallContext` | `value.rs:106/120/133` | 树遍历闭包 / native 函数（复用） |
| `VirtualMachine` | `vm.rs:34` | 运行时壳子，加 `exec_mode`/`compile_cache` |
| `Environment` | `environment.rs:21` | VM 只用 root 的 globals/this/module_dir |
| `Awaitable` / `EventLoop` | `object/awaitable.rs` / `event_loop.rs` | 阶段 9 复用，不改 |
| `ImporterFn` | `vm.rs:30` | 模块加载回调，阶段 8 复用 |
| `register_globals` | `evaluator/builtins.rs` | 内建全局注入，VM 完全复用 |

---

**本计划的根本约束**：**VM 必须承接所有现有功能，全量交付，契约门不可跳过。** 每个阶段的验收是 VM 单跑全绿，不是双跑对齐。任何未覆盖的 AST 节点 = 阻断，不得作为 TODO 留到下一阶段。满足这些约束，VM 才能真正替代树遍历，而非与之长期共存。
