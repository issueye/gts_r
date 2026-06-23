# GoScript (Rust) 架构总览

> 本文整合了字节码 VM、异步运行时、对象系统、并发模型与 GTP 协议的核心设计决策。这些子系统历史上分散在 `EVENTLOOP_IMPROVEMENT_PLAN` / `TOKIO_INTEGRATION` / `AWAITABLE_BRIDGE_SUMMARY` / `GTP_FINAL_REPORT` 等多份文档中,此处收敛为单一真相源。

## 1. 执行管线

```
源码 (.gs)
   │  Lexer
   ▼
Token 流
   │  Parser
   ▼
AST (crate::ast)
   │
   ├──► TreeWalker (evaluator/, legacy fallback, --exec-mode=tree)
   │
   └──► Bytecode VM (默认)
            │  Compiler (bytecode/compiler.rs)
            ▼
        Chunk { code: Vec<u8>, constants, lines, protected_regions }
            │  Interp (bytecode/interp.rs)
            ▼
        Object 求值结果
```

**默认走字节码 VM**(`Session::new()` 写入 `EXEC_MODE_BYTECODE`,`src/runtime/mod.rs`)。`VirtualMachine::exec_mode: AtomicU8` 决定后端;CLI `--exec-mode=tree` 回退树遍历。

### 1.1 为什么需要字节码 VM

树遍历的痛点(诊断于 `evaluator/`):

| 痛点 | 位置 | VM 如何消除 |
|------|------|-------------|
| AST 重复遍历、无编译缓存 | `expressions.rs` 巨型 match | 编译一次,Chunk 复用 |
| 控制流靠抛异常 | `BREAK_SIGNAL="__break__"` | 跳转指令 |
| `return` 靠 `Object::Return(Box<Object>)` | `value.rs` | `Return` opcode |
| 变量查找是字符串哈希 + 父链遍历 | `environment.rs` | 局部槽 / upvalue / 全局名字表 |
| 每个 AST 节点都 `check_timeout` | `eval_core.rs` | 边界检查下沉 |

实测(`tests/bytecode_alloc.rs`,`--ignored`):百万次空 `for` 循环 VM 分配 ≪ 树遍历(树遍历每轮创建 block scope,VM 仅 15 次 vs 1000003 次)。

### 1.2 字节码 VM 组成

| 文件 | 职责 |
|------|------|
| `bytecode/opcode.rs` | 47+ 条栈式指令(算术/比较/逻辑/变量槽/跳转/对象/类/closure/upvalue/match/module/await) |
| `bytecode/chunk.rs` | `Chunk { code, constants, lines, protected_regions }` + 常量池去重 + 反汇编 |
| `bytecode/compiler.rs` | `Program → Chunk`,穷尽匹配全部 18 Stmt + 27 Expr 变体 |
| `bytecode/resolve.rs` | 词法解析 pass:局部槽 / upvalue / 转发 upvalue / 全局 四态绑定 |
| `bytecode/closure.rs` | `FunctionProto`(arity/param_slots/upvalue_desc/is_async/lexical_this)+ `ClosureData` |
| `bytecode/frame.rs` | `CallFrame { ip, proto, slots, upvalues, this, slot_base }` |
| `bytecode/upvalue.rs` | 开放(指向外层栈槽)/闭合(迁移到 `Rc<RefCell<Object>>`)两态 upvalue |
| `bytecode/call.rs` | 闭包调用约定,native↔VM 互调桥接(`apply_function` 的 `Object::Closure` 臂) |
| `bytecode/class.rs` | `NewClass`/`super.method` 下沉到 VM 侧 `Class` 组装 |
| `bytecode/interp.rs` | dispatch loop,维护 `open_upvalues: BTreeMap<slot, Vec<Rc<Upvalue>>>` |

契约验收标准是 **VM 单跑全绿**(非双跑对齐),详见 [`bytecode-vm-development-plan.md`](bytecode-vm-development-plan.md) 与 [`bytecode-vm-todo.md`](bytecode-vm-todo.md)。当前阶段 0–11 已完成(全量 AST 覆盖、性能不劣于树遍历、默认 Bytecode)。

---

## 2. 对象系统

### 2.1 `Object` 枚举与所有权模型

`Object`(定义于 `object/value.rs`)是脚本值的统一表示:

```
Null / Undefined / Boolean / Number(f64) / String
Array(Rc<RefCell<ArrayData>>)
Hash(Rc<RefCell<HashData>>)
Map(Rc<RefCell<MapData>>)  / Set(Rc<RefCell<SetData>>)
Function(Rc<FunctionData>)  / Closure(Rc<ClosureData>)   ← VM 闭包
Builtin(Rc<Builtin>)  / BuiltinFn(Rc<dyn Fn>)
Class(Rc<ClassData>) / Instance(Rc<RefCell<InstanceData>>)
Promise(Rc<RefCell<PromiseData>>)
Regexp / Return / Error / ...
```

**硬约束:VM 单线程,`Object` 非 `Send`。** 10+ 变体用 `Rc<RefCell<T>>`,`Environment` 持有 `Rc<VirtualMachine>`,`BuiltinFn` 是 `Rc<dyn Fn>`。用户 handler 函数对象**无法跨线程传递**——这是类型系统层面的限制。把 `Object` 系统从 `Rc` 改成 `Arc`、`RefCell` 改成 `Mutex` 是 VM 级重构(约 400 处改动点),**项目已明确拒绝此路线**,以保持 native 求值路径零开销。

这个约束决定了整个并发模型(见 §4)。

### 2.2 关键子系统

- **Environment** (`object/environment.rs`):词法作用域链,`get()` 走字符串哈希 + 父链。VM 通过 `LoadName`/`AssignName` 动态查找或局部槽直访。
- **Promise** (`object/promise.rs`):状态机 + settlement continuation。`then/catch/finally` 对 pending Promise 返回下游 Promise 而不阻塞等待,使 async Web handler 成为可能。
- **Awaitable** (`object/awaitable.rs`):poll-based 异步抽象(`poll(&self, waker) -> PollResult`),类似 Rust Future。`EventLoop`(`object/event_loop.rs`)驱动任务调度 + `TimerWheel`(`object/timer_wheel.rs`)定时器。
- **I/O 多路复用** (`object/io_selector/`):跨平台抽象 `IoSelector` trait。
  - Linux: epoll(边缘触发)
  - macOS/BSD: kqueue
  - Windows: IOCP
  - Fallback: poll
  支持 100K+ 并发连接,~2ms 延迟。

---

## 3. 异步运行时

### 3.1 双运行时与 feature gate

```
[features]
default = ["tokio"]
tokio = ["dep:tokio", "dep:reqwest"]
```

- **Native runtime**(`async_runtime/native.rs`):单线程,`EventLoop` + `TimerWheel` + `IoSelector`,无外部依赖。
- **Tokio runtime**(`async_runtime/tokio_rt.rs`):多线程 I/O,默认启用。`Session::new()` 在 tokio feature 下创建 `TokioRuntime`。

**选择 feature flag 而非 Runtime trait 抽象**(已拒绝 trait 方案),理由:零开销抽象、每个 runtime 独立优化、代码更简单。

### 3.2 VM 线程与 Tokio 的桥接

由于 `Object` 非 `Send`,**Tokio 不能直接执行 GTS VM**。解法是 **completion queue 消息传递模型**:

```
VM 线程                         Tokio 线程
   │                                │
   │  await http.requestAsync()     │
   ├──────── registration ────────► │  spawn tokio task
   │  (Promise id, completion tx)   │  (HTTP / TCP / timer)
   │                                │
   │  wait_async() 阻塞             │
   │  drain completion queue ◄──── │  send owned Send data
   │  resolve/reject Promise        │  ({ status, headers, body })
   │  恢复 bytecode frame           │
   ▼                                ▼
```

跨线程**只允许**传递 owned `Send` 数据:`String`、`Vec<u8>`、status code、headers map、JSON payload、错误字符串。**禁止**跨线程传 `Object` / `Rc` / `EnvRef` / `CallContext`。

实现:`async_runtime/completion.rs`(`AsyncCompletion` / `AsyncCompletionQueue` + `Condvar` 通知),`object/vm.rs`(`drain_async_completions()` 在 VM 线程把 owned completion 转成 `Object` 并 resolve/reject Promise)。`wait_async()` 改为事件循环式 drain,而非单纯 sleep polling。

### 3.3 Awaitable ↔ Future 为什么不能直接转换

```rust
// ❌ 不可能:Awaitable.poll() 需要 Rc<dyn Fn()>,Context.waker() 返回 Arc<Waker>,类型不匹配
impl Future for AwaitableFuture {
    fn poll(&mut self, cx) -> Poll { self.awaitable.poll(cx.waker()) }
}
```

根本原因:`Rc` 非 `Send`、`Object` 用 `Rc<RefCell<T>>`、Awaitable trait 设计为单线程。因此采用 §3.2 的消息传递桥接,而非直接转换。

---

## 4. 并发模型

### 4.1 单 VM 单线程

脚本执行、对象读写、Promise resolve 全部在 VM 所在线程完成。Tokio 只负责可 `Send` 的 I/O 工作(socket / HTTP / timer)。

### 4.2 `@std/web` 多 worker(prefork 共享 socket)

`app.listen(port, { workers: N })` 让 N 个 worker 线程并行处理请求。**每个 worker 持有独立 VM 实例**(各自 `Session::new()` + 重新加载脚本 + 注册 routes),绕开非 `Send` 问题。

```
主线程 (main VM)
  ├─ tiny_http::Server::http("0.0.0.0:port")  绑定
  ├─ Arc<Server> + Arc<AtomicBool shutdown>   共享原语
  ├─ spawn worker 0..N
  │     每个 worker:
  │       ├─ thread_local WEB_WORKER_CTX {server, shutdown, id}
  │       ├─ Session::new()  独立 VM
  │       ├─ run_source(脚本)  重建 routes
  │       └─ accept 循环: recv_timeout(100ms) → web_handle_request()
  ├─ 安装 Ctrl+C handler
  └─ join 所有 worker
```

**关键机制**:worker 重新执行脚本到达 `app.listen()` 时,通过 thread_local 标志 `WEB_WORKER_CTX` 检测到 worker 上下文,**跳过** bind/spawn,直接进入 accept 循环。这样每个 worker 拥有独立构建的 `WebApp`(独立 routes 副本),没有 Object 跨线程。

### 4.3 单 worker 异步不阻塞

`app.listen(port)`(默认单 worker)的 accept loop 已是事件循环驱动:handler 返回 Promise 时挂起响应、继续 accept,在 loop tick 中 drain VM completion。慢上游请求不会占住整个 worker。

实测(`tests/stdlib_p9_web.rs::web_single_worker_does_not_block_fast_route_while_slow_route_waits`):慢路由等待 delayed upstream 时,同一 worker 的 `/healthz` 在 150ms 内返回。

### 4.4 共享状态语义

**prefork 模型下 worker 间不共享脚本层可变状态。** 每个 worker 是独立 VM,全局变量(如计数器)各维护独立副本。这与 Nginx/Apache prefork 行为一致;跨 worker 共享状态应外置(Redis/DB/消息队列)。`workers: 1` 单 worker 长驻时,共享状态可见。

### 4.5 性能(`docs/web-concurrency-benchmark.md`)

I/O 密集(`sleep 50ms` 模拟),吞吐随 worker 数近线性扩展:

| 配置 | 吞吐(req/s) | 相对串行 |
|------|-----------:|--------:|
| serial | 20 | 1.0× |
| workers=4 | 79 | 4.0× |
| workers=8 | 158 | 7.9× |

CPU 密集受物理核心数限制(workers=4 → 3×,workers=8 → 4× 饱和)。纯瞬时请求(`/fast`,微秒级)并发无益,瓶颈在网络/accept。

---

## 5. GTP 进程间协议

GTP(GoScript Transport Protocol)是跨实例 / 插件交互的 JSON Lines 协议,与 Go 版完全兼容。

```
src/
├── gtp/
│   ├── frame.rs        Frame / Value / GtpError(serde 序列化)
│   ├── codec.rs        JsonlEncoder / JsonlDecoder(缓冲 I/O)
│   ├── transport.rs    Transport trait + StreamTransport<R,W>
│   ├── transports/     stdio.rs(插件进程)/ tcp.rs
│   └── plugin.rs       PluginManager 骨架
└── stdlib/gtp/
    ├── client.rs       @std/gtp/client: connectTcp / call / recv / close / isAlive
    └── server.rs       @std/gtp/server(占位)
```

清晰的四层抽象:**协议层**(Frame/Value)→ **编解码层**(Jsonl)→ **传输层**(Transport trait)→ **应用层**(stdlib API)。新增传输方式只需实现 `Transport` trait,新增功能模块只需加独立文件,**不影响 stdlib 主文件**(遵循"一个原生库一个单元"原则)。

Frame 与 Value 类型系统与 Go 版完全对应,JSON 序列化格式逐字节一致(特殊值 NaN/Infinity 处理对齐)。

---

## 6. 设计原则

整个项目遵循以下原则:

1. **契约驱动全量交付** — VM 验收是"单跑全绿",不是"双跑对齐";未覆盖 AST 节点 = 阻断。
2. **零开销抽象** — 默认构建无不必要的 runtime 开销;feature flag 编译期决策。
3. **一个原生库一个单元** — 避免单文件膨胀;stdlib 模块独立成文件(`stdlib/modules/*.rs`),新功能不污染主文件。
4. **渐进式增强** — 核心功能默认可用,高级能力(I/O 多路复用、多线程)可选。
5. **兼容优先于重构美感** — 内部可 Rust 化,但 CLI 输出、错误类型、模块导出、示例行为对齐 Go 版。
6. **文档驱动开发** — 每个 TODO 必须填可验证证据(fixture 名 / 测试输出 / 文件:行号),禁止"已完成"字样。

---

## 7. 关键文件索引

| 子系统 | 入口 |
|--------|------|
| CLI | `src/bin/gs.rs` |
| Session | `src/runtime/mod.rs` |
| 字节码 VM | `src/bytecode/mod.rs` |
| 树遍历 | `src/evaluator/mod.rs` |
| 对象系统 | `src/object/mod.rs`、`object/value.rs` |
| 异步原语 | `object/awaitable.rs`、`object/event_loop.rs`、`object/promise.rs` |
| I/O 多路复用 | `object/io_selector/` |
| Tokio runtime | `async_runtime/tokio_rt.rs` |
| Completion 桥接 | `async_runtime/completion.rs` |
| 模块系统 | `src/module/mod.rs` |
| 标准库 | `src/stdlib/mod.rs` → `stdlib/modules/` |
| GTP | `src/gtp/mod.rs` |
| Web 框架 | `src/stdlib/modules/web.rs` |

详细进度与剩余工作见 [`development-roadmap.md`](development-roadmap.md);逐项功能对齐见 [`parity-matrix.md`](parity-matrix.md)。
