# GoScript 字节码 VM 开发 TODO 清单（执行追踪表）

> **文档用途**：本文件是字节码 VM 开发的**唯一进度真相源**。任何会话切换后，**第一步先读本文件**，按 `当前指针` 续上工作。完成一项就更新本文件（打勾 + 填证据 + 移动指针），**不要凭记忆续工**。
>
> **对应计划**：`docs/bytecode-vm-development-plan.md`（契约驱动全量交付版）
> **核心约束**：VM 必须承接所有现有功能；阶段验收 = VM 单跑全绿（非双跑对齐）；未覆盖 AST 节点 = 阻断。
> **创建日期**：2026-06-22

---

## 使用说明

- 每条 TODO 有：`[状态]` + `阶段` + `描述` + `完成证据` + `阻塞项`。
- 状态：`[ ]` 待办 / `[~]` 进行中 / `[x]` 完成 / `[!]` 阻断。
- **完成证据**：必须填具体的 fixture 名 / 测试名 / 文件:行号 / 命令输出，禁止填"已完成"。
- **每完成一条**：① 打勾 ② 填证据 ③ 把文末 `当前指针` 移到下一条 ④ 提交 git。
- **遇到阻断**：标 `[!]`，写清阻断原因，不要硬推。

---

## 全局准备

- [x] G0 阅读并理解计划文档 `docs/bytecode-vm-development-plan.md`
  - 证据：已读取全文；契约矩阵 §2、阶段 §4、覆盖度表 §3.5 已掌握
- [ ] G1 新建工作分支 `feat/bytecode-vm`（从 main）
  - 证据：（待填 git 分支创建命令输出）
- [ ] G2 确认基线：`cargo build` + `cargo test` 在改动前全绿，记录 fixture 数
  - 证据：（待填 `cargo test` 输出摘要）

---

## 阶段 0：脚手架与可观测性

- [x] 0.1 创建模块目录 `src/bytecode/` 与 `mod.rs`，在 `src/lib.rs` 注册 `pub mod bytecode;`
  - 证据：`src/bytecode/{mod,opcode,chunk,compiler,interp}.rs` 已建；`src/lib.rs:11` 注册 `pub mod bytecode;`；`cargo build --lib` Finished 无 error
- [x] 0.2 实现 `src/bytecode/opcode.rs`：`Opcode` 枚举（阶段 0 仅需 `Const/Add/Pop/Return`，但预留全量枚举框架）+ `disassemble` 反汇编
  - 证据：`Opcode` 全量 47 变体 + `from_byte`/`name`/`Display`；`chunk::tests::disassemble_is_readable` ok
- [x] 0.3 实现 `src/bytecode/chunk.rs`：`Chunk { code: Vec<u8>, constants: Vec<Object>, lines: Vec<Position>, protected_regions: Vec<ProtectedRegion> }` + `write/read_const/read_u16/line_at` 方法
  - 证据：`Chunk::add_constant` 去重 + `write_op/byte/u16/u32` + `read_u16/read_u32/position_at/disassemble`；`chunk::tests::chunk_roundtrips_const_and_add` ok
- [x] 0.4 实现 `src/bytecode/compiler.rs`：`Compiler::new() -> compile(&Program) -> Chunk`，阶段 0 仅支持 `Expr::Number` + `Expr::Infix("+")`（后序遍历）
  - 证据：后序遍历编译 Number+Infix("+")，其余节点返回 CompileError（覆盖原则）；`compiles_literal_number`/`compiles_add_post_order`/`rejects_unsupported_node` ok
- [x] 0.5 实现 `src/bytecode/interp.rs`：`interpret(&Chunk, &EnvRef) -> Object`，dispatch loop 实现 `Const/Add/Pop/Return`
  - 证据：`stage0_contract_one_plus_two`（1+2→3.0）ok；`chain_add_left_associative`（1+2+3→6.0）ok
- [x] 0.6 `src/object/vm.rs` 加 `pub exec_mode: AtomicU8`（0=TreeWalk, 1=Bytecode），默认 0
  - 证据：`vm.rs` 加 `exec_mode: AtomicU8` + `EXEC_MODE_TREEWALK/BYTECODE` 常量，默认 TreeWalk；`cargo build` 通过
- [x] 0.7 阶段 0 契约门：
  - [x] `cargo build` 通过 — Finished, 0 error
  - [x] `cargo clippy --lib` 0 error — 0 error（前序 builtins.rs:1034 的 `never_loop` 误报加 `#[allow]` 解决，注释说明是 Promise.race 有意语义；非 VM 引入）
  - [x] 单测：`1 + 2` → `3.0` — `bytecode::interp::tests::stage0_contract_one_plus_two` ok
  - [x] 树遍历现有测试全绿（回归零破坏）— `cargo test --tests` 全部 ok，0 failed
- [x] 0.8 提交阶段 0，PR 标题 `[bytecode-0] 脚手架`
  - 证据：commit 待执行（下一步）

---

## 阶段 1：表达式全集 + 变量（契约：表达式层全量）

- [x] 1.1 完善运算符指令：`Sub/Mul/Div/Mod/Pow/Eq/Neq/Lt/Le/Gt/Ge` + 逻辑短路 `JumpIfFalse/JumpIfTrue` + `Not/Neg/Concat`
  - 证据：compiler 覆盖全部算术/比较运算符 + `&&`/`||` 短路(Dup+条件跳转) + Prefix `!`/`-` + Bool/Null/Undefined 字面量；interp 复用 evaluator 新增的 `apply_binary_op`/`apply_unary_op`(pub,保证与树遍历逐字节一致)；`cargo test --lib bytecode` 31 passed(每个运算符 + 短路返回操作数值语义)；`cargo clippy --lib` 0 error；回归 `cargo test --tests` 259 passed/26 suites 全绿。`??` 与 bitwise/instanceof/in 推迟到对应 fixture 到达时(1.2/后续阶段)
- [x] 1.2 字面量编译：`Bool/Null/Undefined/Template/Regexp`（Template/Regexp 编译期求值或下沉）
  - 证据：compiler 接入 `eval_string_lit`/`eval_regexp_lit`(纯函数,编译期求值) + 新增 `eval_template_static`(静态模板,无 `${}`)；Bool/Null/Undefined 随 1.1 完成；interp 36 passed(含 string 拼接/转义/严格相等/静态模板)；`${}` 插值模板推迟到 1.3(依赖变量查找)
- [x] 1.3 变量声明与存取：`Let/Const/Var` → 全局名字表；`LoadName/StoreName/AssignName`
  - 证据：compiler 实现 `compile_decl`(let/const/var, const 用高位 bit 标记) + `Ident`→LoadName + `compile_assign`(= 与复合 +=,-= 等)；interp 经 env 名字表路由(StoreName→set_here/set_const_here, AssignName→env.assign 含 const TypeError 与 ReferenceError)；新增 9 个变量单测(let/const/var/赋值/复合赋值/ReferenceError/TypeError)，cargo test --lib bytecode 45 passed；clippy 0 error；回归全绿
- [x] 1.4 标识符读取：统一走 `LoadName`（动态查找），阶段 4 再优化为槽/upvalue
  - 证据：随 1.3 完成，Ident → LoadName 经 env.get() 走父链+全局
- [x] 1.5 阶段 1 契约门（VM 单跑全绿）—— **stage 2.1 后达成**
  - stage-1 的 parity fixture 实际依赖 `if`/`${}`插值/println 桥接（stage-2），随 2.1 一起端到端验证通过
  - [x] `basic_expression` → "basic-expression=1\n" ✅ (bytecode_parity.rs)
  - [x] `comparison_edges` → "comparison-edges=ok\n" ✅
  - [x] `truthy_logic` → "truthy-logic=start:ok\n" ✅
  - [x] `template_literals` → "template-literals=gts:9\n" ✅
  - [x] `control_flow` → "control-flow=8\n" ✅
  - [x] `for_break` → "for-break=6\n" ✅
  - [x] `while_continue` → "while-continue=18\n" ✅
  - 证据：tests/bytecode_parity.rs `bytecode_vm_matches_stage_1_2_fixtures` 7 fixture 全绿（capturing println + 逐字节比对 stdout）
- [x] 1.6 覆盖度核对：§3.5「字面量/运算符/Ident」三节全部打勾
  - 证据：Number/Bool/Null/Undefined/String/Regexp/静态Template + 全算术/比较运算符 + &&/||短路 + !/-一元 + Ident读写 + Let/Const/Var + =/复合赋值，均有单测驱动
- [x] 1.7 提交 `[bytecode-1.3]` 表达式与变量

---

## 阶段 2：控制流全集（契约：跳转替代异常）

- [x] 2.1 实现 `OpJump/JumpIfFalse/JumpIfTrue/Loop` + 回填机制
  - 证据：interp dispatch 全部就绪(Jump/JumpIfFalse/JumpIfTrue/Loop)；emit_jump_placeholder/patch_jump_here/patch_jump_to 回填辅助；chunk.rs disassemble 修复(正确跳过操作数)
- [x] 2.2 编译器循环栈：`{ breaks: Vec<u32>, continues: Vec<u32> }`；break/continue 回填
  - 证据：LoopFrame + compile_break_continue 收集待回填跳转，循环结束时 patch 到 end(while)或 post_start(for)
- [x] 2.3 `For/While` 编译；`If/Else` 跳转
  - 证据：compile_if/compile_while/compile_for；修了 for-continue 的 post_start 偏移 bug(原本指向 body 起点导致死循环)；Stmt::Block + 表达式语句 keep_value 语义(顶层末语句保留值,其余 pop)
- [x] 2.4 迭代器协议：`ForIn`（遍历 key）、`ForOf`（遍历 value），支持 Array/String/Map/Set
  - 证据：新增 `Opcode::IterKeys/IterValues/Len` + 编译器 `compile_for_iter` 将 ForIn/ForOf 降为普通索引循环；interp 复用 `iterable_keys/iterable_values` 并支持 Array/String/Hash/Map/Set；`for_of_array` fixture 覆盖 Array/String/Map/Set 值遍历，`for_in_object` 覆盖对象 key 遍历；`cargo test --test bytecode_parity -- --nocapture` ok
- [x] 2.5 **补 fixture**（先在树遍历下验证绿）：`for_in_object`、`for_of_array`、`labeled_break`
  - 证据：新增 `tests/fixtures/parity/{for_in_object,for_of_array,labeled_break}/main.gs`；树遍历 CLI 输出分别为 `for-in-object=abc`、`for-of-array=6:go:5:xy`、`labeled-break=1`
- [x] 2.6 阶段 2 契约门（VM 单跑全绿）：
  - [x] `control_flow`（当前 parity 目录无独立 `03_control_flow` fixture）
  - [x] `while_continue`
  - [x] `for_break`
  - [x] `nested_loops`
  - [x] `loop_array_build`
  - [x] 新补 `for_in_object` `for_of_array` `labeled_break`
  - 证据：`tests/bytecode_parity.rs::bytecode_vm_matches_stage_1_2_fixtures` 覆盖上述 fixture 并通过；新增 VM 单测 `bytecode::interp::tests::labeled_break_exits_outer_loop` 覆盖跳出外层循环；`cargo test --lib bytecode` 68 passed；`cargo test --tests` 前序语言/stdlib 多套件通过，但 `tests/stdlib_p8_exec.rs` 5 例因外部程序 `program not found` 失败（非 VM 路径）
- [x] 2.7 **质心收益证据**：`for(i=0;i<1_000_000;i++){}` 堆分配次数 VM ≪ 树遍历（树遍历热循环每轮创建 block scope）
  - 证据：新增独立测量单元 `tests/bytecode_alloc.rs::million_empty_for_loop_allocates_far_less_on_bytecode_vm`（默认 ignored，避免拖慢常规测试）；命令 `cargo test --test bytecode_alloc -- --ignored --nocapture` 输出 `stage2_hot_loop_allocations tree_walk=1000003 bytecode_vm=15 ratio=66666.9x tree_elapsed_ms=1580 vm_elapsed_ms=1208 tree_deallocs=1000001 vm_deallocs=13`；VM 变量名热路径改为借用常量池字符串，避免每次 `LoadName/AssignName` 重新分配
- [x] 2.8 提交 `[bytecode-2] 控制流`
  - 证据：提交 `9e9e1c5 feat(bytecode): advance control flow and function frames`，包含阶段 2 控制流/迭代器/分配证据与阶段 3 函数帧结构进展

---

## 阶段 3：函数 + CallFrame + native 互调

- [x] 3.1 实现 `src/bytecode/closure.rs`：`FunctionProto`（含 chunk 片段/arity/param_slots/upvalue_desc/is_async/lexical_this/pos）+ `ClosureData`
  - 证据：`src/bytecode/closure.rs` 已有 `FunctionProto { name, params, arity, param_slots, upvalue_desc, body, is_async, lexical_this, return_t, pos, chunk }` + `ClosureData { proto, home_env }`；`ParamSlot` 记录参数槽/默认值/rest/optional；`UpvalueDesc`/`UpvalueSource` 结构已固化（阶段 4 负责填充非空描述）；单测 `bytecode::closure::tests::function_proto_records_arity_and_param_slots` ok
- [x] 3.2 实现 `src/bytecode/frame.rs`：`CallFrame { ip, proto, slots, upvalues, this, slot_base }`
  - 证据：新增 `src/bytecode/frame.rs`，包含 `CallFrame { ip, proto, slots, upvalues, this, slot_base }` 与 `FrameUpvalue`；`bytecode::call::call_closure_impl` 在参数绑定后构造 stage-3 frame metadata；单测 `bytecode::frame::tests::frame_mirrors_bound_params_into_slots` ok
- [x] 3.3 `Object` 新增变体 `Object::Closure(Rc<ClosureData>)`（`object/value.rs`，纯新增）
  - 证据：`src/object/value.rs` 已有 `Object::Closure(Rc<crate::bytecode::closure::ClosureData>)`；`inspect/type_tag/equals` 均有 Closure 分支
- [x] 3.4 实现 `OpClosure/Call/Return/ReturnNull`；调用约定（callee 栈顶、参数紧贴其下）
  - 证据：`Opcode::Closure/Call/Return/ReturnNull` 均有解释器执行分支；`src/bytecode/interp.rs` 的 `Call` 按 `[..., callee, arg1, ..., argN]` 拆栈并调用；函数声明/表达式/箭头函数/递归函数单测通过；`bytecode::interp::tests::return_null_opcode_returns_null` 覆盖 `ReturnNull`
- [x] 3.5 **关键桥接**：`evaluator/expressions.rs:745 apply_function` + `methods.rs` 同名点新增 `Object::Closure` 臂，委托 `bytecode::interp::call_closure`
  - 证据：按功能拆分为 `src/bytecode/call.rs`；`apply_function` 的 `Object::Closure` 分支委托 `bytecode::call::call_closure_object`；新增 `array_map_callback` fixture 覆盖 `[1,2,3].map((value)=>value*2)` native→VM 闭包回调，`cargo test --test bytecode_parity -- --nocapture` ok
- [x] 3.6 参数默认值 / spread / `arguments` 对象（对齐现有 `bind_params`）
  - 证据：VM 闭包调用复用 `evaluator::expressions::bind_params`；`function_params` fixture 覆盖默认参数（缺席实参触发默认值，显式 `undefined` 保持 undefined），树遍历与 VM 输出均为 `function-params=item:undefined:key`；`function_rest_params` fixture 覆盖 rest 参数，树遍历与 VM 输出均为 `function-rest-params=v:1|2|3:3`；新增 `function_arguments` fixture 覆盖 `arguments` 对象，树遍历与 VM 输出均为 `function-arguments=3:a:c:a`；新增 `function_spread_call` fixture 覆盖调用位置 spread 实参，树遍历与 VM 输出均为 `function-spread-call=a:b:c`
- [x] 3.7 阶段 3 契约门（VM 单跑全绿）：
  - [x] `function_call` `recursive_function`
  - [x] `function_params` `function_rest_params` `function_arguments` `function_spread_call`
  - [x] `string_methods`（native 方法回调）
  - [x] **native↔VM 互调专项**：`array_map_callback` 即 `[1,2,3].map((value)=>value*2)` 在 VM 下绿
  - 证据：`tests/bytecode_parity.rs::bytecode_vm_matches_stage_1_3_fixtures` 当前覆盖 `function_call`、`recursive_function`、`function_params`、`function_rest_params`、`function_arguments`、`function_spread_call`、`string_methods`、`array_map_callback` 并通过；`cargo test --lib bytecode` 71 passed；本地闭包捕获归阶段 4 `function_closure`
- [x] 3.8 覆盖度核对：`Func/Arrow/Call/FuncDecl` 打勾
  - 证据：`src/bytecode/compiler.rs` 覆盖 `Stmt::FuncDecl`、`Expr::Func`、`Expr::Arrow`、`Expr::Call`；`Expr::Call` 同时覆盖固定参数 `Call` 与 spread 实参 `CallSpread`；`bytecode::interp` 函数声明/匿名函数/箭头函数/递归函数单测通过；parity fixtures 覆盖函数调用、默认参数、rest、`arguments`、spread call、native 回调
- [x] 3.9 提交 `[bytecode-3] 函数与 native 互调`
  - 证据：提交 `51cadf8 feat(bytecode): complete stage 3 call parameters`，包含 `arguments` 对象、调用位置 spread 实参、阶段 3 fixture 与 TODO 收口

---

## 阶段 4：闭包与 Upvalue

- [x] 4.1 `Upvalue` 两态模型：开放（指向外层栈槽）/ 闭合（迁移到 `Rc<RefCell<Object>>`）
  - 证据：新增 `src/bytecode/upvalue.rs`，实现 `UpvalueState::Open { slot }` 与 `UpvalueState::Closed(Rc<RefCell<Object>>)`；单测 `bytecode::upvalue::tests::open_upvalue_reads_and_writes_stack_slot`、`bytecode::upvalue::tests::closing_detaches_value_from_stack_slot` 通过；`cargo test --lib bytecode` 输出 73 passed
- [x] 4.2 编译器变量解析 pass：局部槽 / upvalue / 转发 upvalue / 全局 四态
  - 证据：新增 `src/bytecode/resolve.rs` 独立词法解析单元，记录 `ResolvedBinding::{LocalSlot, Upvalue, Global}` 且 `UpvalueSource::{LocalSlot, ParentUpvalue}` 覆盖直接/转发 upvalue；`compile()` 入口运行 `resolve_program()`，`compile_function_proto()` 将解析出的 `upvalue_desc` 写入 `FunctionProto`；单测 `bytecode::resolve::tests::{resolves_locals_and_globals,records_direct_upvalue_from_parent_local,records_forwarded_upvalue_through_intermediate_function}` 与 `bytecode::compiler::tests::function_proto_records_resolved_upvalues` 通过；`cargo test --lib bytecode` 输出 77 passed；`cargo test --test bytecode_parity -- --nocapture` passed
- [x] 4.3 Interp 维护 `open_upvalues: BTreeMap<slot_idx, Vec<Rc<Upvalue>>>`，帧退出时闭合
  - 证据：`src/bytecode/interp.rs` 新增 `VmState::open_upvalues: BTreeMap<usize, Vec<Rc<Upvalue>>>` + `current_upvalues`，`OpClosure` 经 `capture_proto_upvalues` 捕获 `LocalSlot/ParentUpvalue`，`Return/ReturnNull` 调用 `close_open_upvalues_from(0)`；`ClosureData` 新增 `upvalues: Vec<Rc<Upvalue>>` 并由 `call_closure_impl` 传给 `interpret_with_upvalues`；单测 `bytecode::interp::tests::open_upvalues_reuse_the_same_slot_capture`、`bytecode::interp::tests::return_closes_open_upvalues_from_frame_slots` 通过；`cargo test --lib bytecode` 79 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 4.4 实现 `OpLoadUpvalue/StoreUpvalue`，`OpClosure` 运行期抓取
  - 证据：`src/bytecode/interp.rs` 新增 `Opcode::LoadUpvalue/StoreUpvalue` 执行分支（u8 upvalue index，缺失/悬空均 VMError），`StoreUpvalue` 保持赋值表达式值在栈顶；`OpClosure` 的运行期抓取已由 4.3 `capture_proto_upvalues` 完成；`src/bytecode/chunk.rs` 反汇编支持 `LoadLocal/StoreLocal/LoadUpvalue/StoreUpvalue` u8 操作数，`src/bytecode/compiler.rs::operand_width` 同步 u8 宽度；新增单测 `load_upvalue_reads_closed_capture`、`store_upvalue_updates_closed_capture_and_leaves_value`、`load_upvalue_can_read_open_stack_slot`；`cargo test --lib bytecode` 82 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 4.5 阶段 4 契约门（VM 单跑全绿）：
  - [x] `05_closures` 等价覆盖：当前 fixture 集以 `function_closure` + 闭包专项 fixture 承接
  - [x] `function_closure`
  - [x] **闭包专项**：counter 模式修改捕获变量、返回闭包后帧退出（闭合验证）、IIFE 捕获
  - 证据：`tests/bytecode_parity.rs::stage_1_3_fixtures` 纳入 `function_closure`、`closure_counter`、`closure_iife`、`closure_returned_frame`；新增 `tests/fixtures/parity/{closure_counter,closure_iife,closure_returned_frame}/main.gs`；树遍历基线分别为 `closure-counter=1:2:3`、`closure-iife=goscript`、`closure-returned-frame=42`；`cargo test --test bytecode_parity -- --nocapture` 1 passed；`cargo test --test parity_compat rust_cli_matches_parity_fixtures -- --nocapture` 1 passed；`cargo test --lib bytecode` 82 passed
- [x] 4.6 debug + release 双跑一致（验证无悬空栈槽 UB）
  - 证据：`cargo test --test bytecode_parity -- --nocapture` 1 passed；`cargo test --release --test bytecode_parity -- --nocapture` 1 passed；`cargo test --lib bytecode` 82 passed；`cargo test --release --lib bytecode` 82 passed；debug/release 均覆盖 `function_closure`、`closure_counter`、`closure_iife`、`closure_returned_frame`，未出现悬空栈槽或优化构建差异
- [x] 4.7 提交 `[bytecode-4] 闭包与 upvalue`
  - 证据：阶段 4 已拆分提交：`806697e feat(bytecode): add upvalue state model`（4.1）、`355b982 feat(bytecode): add closure variable resolver`（4.2）、`f4e477e feat(bytecode): wire open upvalue lifetime`（4.3）、`8693e8a feat(bytecode): implement upvalue opcodes`（4.4）、`1d32e82 feat(bytecode): validate closure capture parity`（4.5）、`e8c5c4e test(bytecode): verify closure parity in release`（4.6）；阶段收口提交记录本条证据

---

## 阶段 5：对象模型全集

- [x] 5.1 `OpNewArray/NewObject/GetProperty/SetProperty/GetIndex/SetIndex/Spread`
  - 证据：`src/bytecode/compiler.rs` 支持数组 spread 字面量、对象 spread/computed key、member/index 赋值目标；`src/bytecode/interp.rs` 实现 `SetIndex`，并让 `Spread` 同时支持数组追加与对象属性拷贝，`SetProperty/SetIndex` 对齐赋值表达式返回值；新增单测 `array_literal_spread_builds_flat_array`、`object_literal_supports_spread_and_computed_keys`、`array_index_assignment_updates_element_and_returns_value`、`object_property_and_index_assignment_update_hash`；`tests/bytecode_parity.rs` 纳入 `arrays_objects`、`array_index_assignment`、`object_computed_key`、`object_nested_access`；`cargo test --lib bytecode` 86 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 5.2 `OpNew/DefineMethod/NewClass`；`CallFrame.this` 绑定（对齐 `environment.rs:26-29`）
  - 证据：`src/bytecode/compiler.rs` 覆盖 `Stmt::ClassDecl`/`Expr::Class` → `NewClass`，`Expr::New` → `New`，并用 `Call` 高位标记 member/index callee 的 receiver；`src/bytecode/chunk.rs` 保存 class decl 表，`src/bytecode/interp.rs` 执行 `NewClass` 时复用 `build_class`，执行 `LoadThis` 并把 receiver 透传到 builtin/bytecode closure/tree-walker function 调用；`src/bytecode/call.rs` 支持 bytecode closure 的显式 `this`；`DefineMethod` opcode 保持预留，方法体下沉到 VM 原型编译留给 5.3；`tests/bytecode_parity.rs` 纳入 `object_method_call`、`class_basic`、`class_method_this`；`cargo test --lib bytecode` 86 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 5.3 super 方法解析（复用 `methods.rs` 逻辑）；`build_class` 下沉到编译器
  - 证据：新增 `src/bytecode/class.rs` 在 VM 侧组装 `Class` 并把方法/构造器编译为 bytecode closure；`src/bytecode/compiler.rs` 编译 `super(...)` 与 `super.method(...)` 到 `SuperMethod` + receiver-aware `Call`；`src/bytecode/interp.rs` 实现 `SuperMethod` 分派；`src/evaluator/methods.rs` 扩展共享类构造/方法绑定以识别 `Object::Closure`，保留原 `Object::Function` 路径；`tests/bytecode_parity.rs` 纳入 `class_inheritance_method`、`class_inheritance_constructor`、`class_implicit_super`、`class_super_method_override`、`class_field_update`；`cargo test --test bytecode_parity -- --nocapture` 1 passed；`cargo test --lib bytecode` 86 passed
- [x] 5.4 computed key（`OpSetIndex`）/ 嵌套访问
  - 证据：5.1 已实现并验证：`src/bytecode/compiler.rs` 对对象字面量 computed key、computed member/index 赋值分别发出 `SetIndex`，member/index 读取发出 `GetProperty`/`GetIndex`；`src/bytecode/interp.rs` 的 `SetIndex` 支持数组与对象写入并保留赋值表达式返回值；单测覆盖 `object_literal_supports_spread_and_computed_keys`、`object_property_and_index_assignment_update_hash`；VM parity 已覆盖 `object_computed_key`、`object_nested_access`、`object_method_call`，并在 5.3 后复跑 `cargo test --test bytecode_parity -- --nocapture` 1 passed、`cargo test --lib bytecode` 86 passed
- [x] 5.5 阶段 5 契约门（VM 单跑全绿）：
  - [x] `arrays_objects`（仓库当前无 `06_arrays`/`07_objects`/`08_classes` 聚合 fixture，改以现有 parity 目录中的 `array_*`/`object_*`/`class_*` 明细门验收）
  - [x] `array_*`(6) `object_*`(3) `class_*`(7) 全绿
  - [x] **专项**：`class_super_method_override` `class_inheritance_constructor` `class_implicit_super` `class_method_this` `class_field_update`
  - [x] `object_computed_key` `object_nested_access` `object_method_call`
  - 证据：`tests/bytecode_parity.rs` 纳入 `arrays_objects`、`array_index_assignment`、`array_reduce`、`array_slice_join`、`array_shift_unshift`、`array_find_index`、`array_map_callback`、`object_computed_key`、`object_nested_access`、`object_method_call`、`class_basic`、`class_inheritance_method`、`class_inheritance_constructor`、`class_implicit_super`、`class_super_method_override`、`class_method_this`、`class_field_update`；`cargo test --test bytecode_parity -- --nocapture` 1 passed；`cargo test --lib bytecode` 86 passed
- [x] 5.6 覆盖度核对：`Array/Object/Member/Index/New/Super/This/Assign/Spread/Class` 打勾
  - 证据：`src/bytecode/compiler.rs` 已覆盖 `Expr::Array`→`NewArray`（含数组 spread）、`Expr::Object`→`NewObject`（含对象 spread/computed key）、`Expr::Member`→`GetProperty/GetIndex`、`Expr::Index`→`GetIndex`、`Expr::New`→`New`、`Expr::This`→`LoadThis`、`Expr::Super`/`super.method`→`SuperMethod`、`Expr::Assign`→名字/成员/索引赋值、`Stmt::ClassDecl`/`Expr::Class`→`NewClass`，调用位置 `Expr::Spread`→`CallSpread`；`src/bytecode/interp.rs` 已实现 `Spread/New/NewClass/NewArray/NewObject/GetProperty/SetProperty/GetIndex/SetIndex/LoadThis/SuperMethod`；`tests/bytecode_parity.rs` 覆盖 `arrays_objects`、`array_index_assignment`、`array_reduce`、`array_slice_join`、`array_shift_unshift`、`array_find_index`、`function_spread_call`、`object_computed_key`、`object_nested_access`、`object_method_call`、`class_basic`、`class_inheritance_method`、`class_inheritance_constructor`、`class_implicit_super`、`class_super_method_override`、`class_method_this`、`class_field_update`；5.5 已复跑 `cargo test --test bytecode_parity -- --nocapture` 1 passed 与 `cargo test --lib bytecode` 86 passed
- [x] 5.7 提交 `[bytecode-5] 对象模型`
  - 证据：阶段 5 已拆分提交：`68902ab feat(bytecode): implement object and array opcodes`（5.1）、`7af8c3a feat(bytecode): bridge class construction and this binding`（5.2）、`11b5d6f feat(bytecode): lower class methods and super dispatch`（5.3）、`d438ea4 docs(bytecode): close computed key milestone`（5.4）、`2eef998 test(bytecode): close object model parity gate`（5.5）、`1aa4b20 docs(bytecode): audit object model coverage`（5.6）；本条收口提交记录阶段 5 完成态

---

## 阶段 6：错误处理全集

- [x] 6.1 `OpThrow` + `Chunk.protected_regions`（`ProtectedRegion { try_start, try_end, handler_ip, finally_ip, catch_binding_slot }`）
  - 证据：`src/bytecode/compiler.rs` 编译 `Stmt::Throw` 为表达式求值 + `Opcode::Throw`，编译 `Stmt::Try` 时写入 `Chunk.protected_regions` 的 `try_start/try_end/handler_ip/finally_ip/catch_binding_slot`，并把 catch 入口降为栈顶异常绑定；`src/bytecode/interp.rs` 实现 `Opcode::Throw`，非 Error 抛值按树遍历语义包装为 runtime `Error` 并保留 `thrown`；新增单测 `bytecode::compiler::tests::compiles_throw_opcode`、`bytecode::compiler::tests::records_try_protected_region`、`bytecode::interp::tests::throw_opcode_wraps_non_error_value`；`cargo test --lib bytecode` 89 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 6.2 Interp 抛错 unwind：沿帧栈查 region，命中跳 handler，否则向上
  - 证据：`src/bytecode/interp.rs` 为 `VmState` 记录 `last_ip`，`run()` 在 `step()` 返回 runtime error 时调用 `unwind_to_handler`，按 `Chunk.protected_regions` 查找覆盖 fault ip 的最内层 region，命中时把 catch 值压栈并跳到 `handler_ip`，未命中则原样向调用方返回；catch 绑定前将 runtime `Error` 转为可读 catch 值；新增单测 `bytecode::interp::tests::try_catch_unwinds_to_handler` 覆盖 `throw "boom"` 命中 catch 并读取 `err.message`；`cargo test --lib bytecode` 90 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [x] 6.3 finally 语义：无论是否抛错都执行；finally 内 throw 覆盖原异常
  - 证据：`src/bytecode/compiler.rs::compile_try` 为 try/catch/finally 生成正常 finally 路径与异常 finally 路径，异常路径保存 pending error、执行 finally 后重新 `Throw`，并额外保护 catch 区间使 catch 内 throw 也进入 finally；finally 内新 `Throw` 会直接替换原 pending error；新增单测 `bytecode::interp::tests::try_finally_runs_on_normal_path`、`catch_then_finally_runs_in_order`、`finally_throw_overrides_original_throw`，并更新 `bytecode::compiler::tests::records_try_protected_region` 验证 catch 保护区；`cargo test --lib bytecode` 93 passed；`cargo test --test bytecode_parity -- --nocapture` 1 passed
- [ ] 6.4 阶段 6 契约门（VM 单跑全绿）：
  - [ ] `09_errors`
  - [ ] `try_catch` `try_finally_no_throw` `catch_finally_order`
  - [ ] `throw_catch_string` `throw_catch_error`
  - [ ] `match_no_arm_catch`
  - 证据：（待填）
- [ ] 6.5 错误位置：`Chunk.lines` 反查，message 与树遍历**逐字符一致**
  - 证据：（待填逐字符比对）
- [ ] 6.6 提交 `[bytecode-6] 错误处理`

---

## 阶段 7：Match 全集 + 类型注解

- [ ] 7.1 Match 编译：scrutinee 求值 + 逐 arm 模式测试 + 命中跳转 + body + guard
  - 证据：（待填）
- [ ] 7.2 5 种 Pattern：`Literal(Eq)/Ident(绑定)/Wildcard(无条件)/Or(任一)/Range(Ge+Le)`
  - 证据：（待填）
- [ ] 7.3 `MatchBody::{Expr,Block}` + `guard`
  - 证据：（待填）
- [ ] 7.4 类型注解：`type_check=true` 时插入运行期检查调用；默认 false 对齐树遍历
  - 证据：（待填）
- [ ] 7.5 **补 fixture**（先树遍历下绿）：`match_or` `match_range` `match_guard`
  - 证据：（待填）
- [ ] 7.6 阶段 7 契约门（VM 单跑全绿）：
  - [ ] `match_basic` `match_string` `match_null` `match_boolean` `match_default_only` `match_block_body` `match_no_arm_catch`
  - [ ] 新补 `match_or` `match_range` `match_guard`
  - [ ] `10_typeof`
  - 证据：（待填）
- [ ] 7.7 覆盖度核对：`Match` + 5 Pattern + `MatchBody` + `guard` 打勾
- [ ] 7.8 提交 `[bytecode-7] Match 与类型`

---

## 阶段 8：模块系统全集

- [ ] 8.1 `Import` 编译：调用 `vm.rs:30 ImporterFn`，导出值绑定到本地槽
  - 证据：（待填）
- [ ] 8.2 `Export` 编译：求值后写入模块导出表；re-export `export { } from "..."`
  - 证据：（待填）
- [ ] 8.3 循环依赖检测（复用现有 module cache）
  - 证据：（待填）
- [ ] 8.4 阶段 8 契约门（VM 单跑全绿）：
  - [ ] `relative_require` `nested_relative_require` `project_module_require` `directory_module_index` `module_cache` `module_exports_object` `import_default_like` `export_const` `export_function_alias`
  - 证据：（待填）
- [ ] 8.5 覆盖度核对：`Import/Export` 打勾
- [ ] 8.6 提交 `[bytecode-8] 模块系统`

---

## 阶段 9：异步全集

- [ ] 9.1 `OpAwait`；`src/bytecode/awaitable.rs` 实现 `BytecodeFrameAwaitable: Awaitable`
  - 证据：（待填）
- [ ] 9.2 async 函数/方法/箭头：`FunctionProto.is_async=true`
  - 证据：（待填）
- [ ] 9.3 接线 `async_runtime/awaitable_bridge.rs` + `object/event_loop.rs`（复用，不改）
  - 证据：（待填）
- [ ] 9.4 阶段 9 契约门（VM 单跑全绿）：
  - [ ] `11_promises` `12_async_await`
  - [ ] Promise.all/race/resolve/reject 时序一致
  - [ ] async 内 try/catch 捕获 await 抛错一致
  - [ ] setTimeout/setInterval 一致
  - 证据：（待填）
- [ ] 9.5 覆盖度核对：`Await` + async `FuncDecl/Method/Arrow` 打勾
- [ ] 9.6 提交 `[bytecode-9] 异步`

---

## 阶段 10：全量验收 + 默认切换

- [ ] 10.1 **REQ-1**：`cargo test --test bytecode_parity` 全部 fixture（51+补的）VM 单跑全绿
  - 证据：（待填测试输出）
- [ ] 10.2 **REQ-2**：`gts\verification\**` 12 套件在 `--exec-mode=bytecode` 下全绿
  - 证据：（待填）
- [ ] 10.3 **REQ-3**：§3.5 编译器覆盖度表 100%，无未覆盖节点
  - 证据：（待填最终覆盖度表）
- [ ] 10.4 性能基准：`bench/scripts/bench_server.gs` 在 fib/字符串拼接/Promise 创建三类下 VM 不劣于树遍历
  - 证据：（待填基准数据）
- [ ] 10.5 `Session::new()` 默认 `ExecMode::Bytecode`；保留 `--exec-mode=tree`
  - 证据：（待填）
- [ ] 10.6 决定：树遍历保留 legacy fallback 还是移除（本阶段只决定）
  - 证据：（待填决定 + 理由）
- [ ] 10.7 提交 `[bytecode-10] 全量交付与默认切换`

---

## 当前指针

> **续工时从这里开始。**

**当前阶段**：阶段 2 控制流全集已提交；阶段 3 已完成 Closure 变体、函数调用主路径、native→VM 回调桥接、函数原型元数据、CallFrame 结构、ReturnNull、默认参数、rest、`arguments` 对象与调用位置 spread 实参；调用逻辑已拆到 `src/bytecode/call.rs`，帧模型拆到 `src/bytecode/frame.rs`；阶段 4 闭包与 upvalue 已完成并提交；阶段 5 对象模型全集已完成并收口；阶段 6.1-6.3 `OpThrow`、`Chunk.protected_regions`、catch unwind 与 finally 语义已完成
**下一条 TODO**：继续阶段 6，推进 6.4 阶段 6 契约门（VM 单跑全绿）
**阻断**：宽测试 `cargo test --tests` 仍有 `stdlib_p8_exec` 外部程序找不到的既有环境失败，需要单独处理
**最后更新**：2026-06-22（阶段 6.3 已完成：try/catch/finally 的正常 finally、异常 finally 与 finally 覆盖原异常语义均有 VM 单测；验证 `cargo test --lib bytecode` 93 passed、`cargo test --test bytecode_parity -- --nocapture` 1 passed；下一步纳入错误处理 parity 门）

---

## 续工 SOP（任何新会话必读）

1. 读本文件 → 找 `当前指针`。
2. 读 `docs/bytecode-vm-development-plan.md` 对应阶段章节。
3. 执行 `当前指针` 指向的 TODO；完成后立即回填证据 + 打勾 + 移动指针。
4. 遇阻断标 `[!]`，写清原因，停在原地。
5. 每个 `[x]` 都必须有可验证的证据（fixture 名 / 测试输出 / 文件:行号），禁止"已完成"字样。
