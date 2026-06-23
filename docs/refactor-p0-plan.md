# P0 重构方案：异步死代码清理 + VM God Object 拆分

> 状态：**待审阅**（未执行）。本文件是方案文档，审阅通过后再决定是否实施。
> 依据：`docs/整体代码分析报告`（gts_r 项目代码分析）。
> 原则：每一步都给出 **删什么 / 改什么 / 为什么安全 / 风险 / 验证**，可独立验证后再进入下一步。

---

## 0. 背景与目标

经全项目分析，`gts_r` 的**真实生产异步路径**极其简洁：

```
stdlib 异步操作（HTTP/web）
  → vm.create_async_completion_promise() + vm.async_completion_sender()
  → std::thread::spawn 或自带 tokio runtime 执行 owned I/O
  → sender.resolve(id, data) → AsyncCompletionQueue（Mutex+Condvar）
  → VirtualMachine::wait_async() 在 VM 线程排空 → Promise.resolve/reject
```

而在此路径**之外**，存在一套完整的、基于 poll 的异步抽象层（`Awaitable`/`Waker`/`EventLoop`/`TimerWheel`/`io_selector`）以及一个 tokio 桥接层（`awaitable_bridge.rs`）和一个未被使用的 `Session.tokio_runtime`。这些代码：

- **默认构建下不编译**（`io_selector` 被 `#[cfg(not(feature="tokio"))]` 门控，而 tokio 是默认 feature）；
- **或在生产代码中无任何调用者**（仅被自身测试 / doc 别名 / Cargo example 引用）；
- 约 **1,200+ 行** 精心编写却未接入生产路径的代码。

**目标**：删除确认无生产引用的死代码，并把 `VirtualMachine` 中与 HTTP/stream 响应转换耦合的 ~220 行（且与 `stdlib/modules/stream.rs` 高度重复）的逻辑抽离 / 去重，让 `VirtualMachine` 回归"配置 + 全局 + 异步协调"的核心职责。

**两个 P0 项互相独立，可分别实施 / 分别验证。** P0-A（异步死代码）风险主要在编译期，删完能编过 + 测试过即安全；P0-B（VM 拆分）涉及行为等价性验证（有轻微重复逻辑差异）。

---

## 1. 执行前确认清单（已有证据，列出以备复核）

以下结论均已通过跨 `src/` + `tests/` 的 grep + 阅读交叉验证。实施前可再次复核。

| # | 结论 | 关键证据 |
|---|---|---|
| C1 | `spawn_blocking_gts` / `AsyncCoordinator` / `SerializedResult` 在生产代码零调用，仅自身测试 + `examples/awaitable_bridge_demo.rs` | grep 全树仅命中定义文件、`mod.rs` re-export、example |
| C2 | `TcpConnectAwaitable` / `TcpReadAwaitable` / `TcpWriteAwaitable` 零外部调用；stdlib 网络代码一律用原生 `std::net::TcpStream` | `net_socket_client.rs` / `net_ws_*.rs` / `helpers/net.rs` / `gtp/transports/tcp.rs` 均 `use std::net::TcpStream` |
| C3 | `TimerAwaitable` 零调用；`TimerWheel` 唯一非自身调用者是**死代码** `EventLoop`（见 C4） | `timer_wheel.rs` 自身 + `event_loop.rs:51,298` |
| C4 | `EventLoop::new()` 在**任何**非测试 / 非文档 / 非示例代码中均不构造；`create_runtime()`（`native.rs:70`）也仅自身测试调用 | `event_loop.rs:333,353,376`(test) / `io_selector_integration_test.rs:23,70`(test) / `bytecode/awaitable.rs:99`(test) / `native.rs:80`(test) |
| C5 | `Awaitable` trait / `PollResult` / `Waker` / `WakerRegistry` 仅在"死代码簇"内流转；`WakerRegistry` 唯一消费者是 `Promise`（且 Promise 生产不经 `poll()`，走完成队列） | `promise.rs:22,39,64` |
| C6 | `BytecodeFrameAwaitable` 零外部调用；`OpAwait` 实际经 `vm.wait_async()`，不用此类型 | `bytecode/interp.rs:1269` → `vm.wait_async()` |
| C7 | `Session.tokio_runtime()` 零调用；`has_tokio()` 仅 example；该字段构造后从不用于驱动脚本；HTTP 客户端自建独立 runtime | `runtime/mod.rs:91-92`（无调用）/ `net_http_client.rs:77-92`（自带 OnceLock runtime） |
| C8 | `vm.rs:252-469` 的 10 个函数/结构均为私有、纯函数（无 `self`/无 VM 私有字段访问），唯一调用点是 `drain_async_completions:193` | grep 全树，引用全部在 `vm.rs:187-469` 内 |
| C9 | `vm.rs` 的 stream 逻辑与 `stdlib/modules/stream.rs` 高度重复（`StreamState`/`stream_from_text`/`stream_read*` 几乎逐行对应） | 对比 `stream.rs:49-183` 与 `vm.rs:313-469` |
| C10 | `vm.rs` 的 `async_http_response_to_object` 与 `net_http_client.rs:440`（同名私有 fn）几乎相同；`async_http_stream_response_to_object` 对应 `net_http_client.rs:531 build_http_stream_response` | 对比两文件 |

---

## 2. P0-A：异步死代码清理

### 总览：删除清单

按"删除后需同步处理的 re-export / mod 声明"分组。**建议按子步骤顺序执行，每步单独编译 + 测试**。

| 子步骤 | 删除文件 | 删除/修改的声明 | 行数（约） |
|---|---|---|---|
| A1 | `src/async_runtime/awaitable_bridge.rs` | + `mod.rs` 中 `pub mod awaitable_bridge;`（`#[cfg(feature="tokio")]`）、re-export `:57` | 275 |
| A2 | `src/bytecode/awaitable.rs` | + `bytecode/mod.rs:11 pub mod awaitable;` | ~110 |
| A3 | `src/object/io_awaitable.rs` + `src/object/io_selector.rs` + `src/object/io_selector/`（epoll/kqueue/poll/iocp 4 文件） | + `object/mod.rs:8-9 io_selector`（cfg 门控）+ `:22 io_awaitable` re-export + `:24 io_selector` re-export | ~960 |
| A4 | `src/object/event_loop.rs` + `src/object/timer_wheel.rs` | + `object/mod.rs:6 event_loop`、`:11 timer_wheel`、`:26 TimerAwaitable/TimerWheel` re-export | ~580 |
| A5 | `src/object/awaitable.rs`（trait/PollResult/Waker/WakerRegistry） | + `object/mod.rs:4 awaitable`、`:19 Awaitable/PollResult/Waker/WakerRegistry` re-export；**需先处理 Promise 对其的依赖（见 A5 细节）** | ~110 |
| A6 | `src/async_runtime/native.rs` + `tokio_rt.rs` 中的"桥接残留"，`Session.tokio_runtime` 字段及两个访问器 | 见 A6 细节（tokio_rt.rs 不能整删，详见说明） | ~120 + 字段 |

**合计净删约 2,000+ 行**（含 4 个 io_selector 平台文件 ~750 行 + 其余）。

> 注意：删除 `PollResult` 会牵连 `tokio_rt.rs:26,94,110-115`（`BridgedResult::from_poll_result`）和已删的 `awaitable_bridge.rs`。因此 **A1 必须先于 A5**，且 A6 处理 `tokio_rt.rs` 时要同步删掉 `BridgedResult`（若其也无外部调用）。顺序见 §2.7。

---

### A1. 删除 `awaitable_bridge.rs`

**删什么**
- 文件 `src/async_runtime/awaitable_bridge.rs`（275 行）。
- `src/async_runtime/mod.rs:43-44`：
  ```rust
  #[cfg(feature = "tokio")]
  pub mod awaitable_bridge;
  ```
- `src/async_runtime/mod.rs:56-57`：
  ```rust
  #[cfg(feature = "tokio")]
  pub use awaitable_bridge::{spawn_blocking_gts, AsyncCoordinator, SerializedResult};
  ```

**可选附带**
- `examples/awaitable_bridge_demo.rs`：该 example 唯一引用即将删除的符号。建议一并删除（或在 example 顶部加 `//FIXME: 待 awaitable_bridge 恢复` 暂留——不推荐）。删 example 不会影响 `gts` 库 / `gs` bin。

**为什么安全**（证据 C1）
- `spawn_blocking_gts` / `AsyncCoordinator` / `SerializedResult` 的全部外部引用 = `mod.rs` re-export + `examples/awaitable_bridge_demo.rs` + 自身 `#[cfg(test)]`。
- 文件自身（`:196-224`）明确写道："Converting Awaitable → Future is fundamentally problematic"——作者已承认该桥接路线不可行。
- 无任何 stdlib / runtime / object / bytecode 生产代码 import 这些符号。

**风险**：无。纯删除未使用代码。

**验证**
1. `cargo build --all-features`（含 `--no-default-features --features tokio` 组合）通过。
2. `cargo test` 通过。
3. 确认 `gts::async_runtime` 的公开 API（被 `object/mod.rs:15-18` re-export 的 `AsyncCompletion*` 系列）未受影响。

---

### A2. 删除 `bytecode/awaitable.rs`

**删什么**
- 文件 `src/bytecode/awaitable.rs`（约 110 行，含 `BytecodeFrameAwaitable` + 2 个测试）。
- `src/bytecode/mod.rs:11`：`pub mod awaitable;`

**为什么安全**（证据 C6）
- `BytecodeFrameAwaitable` 零外部调用；其 `impl Awaitable::poll`（`:36-52`）一次性把整个 chunk 跑完并 memoize——并未真正 poll，抽象无实际价值。
- 真正的 `await` 字节码处理走 `interp.rs:1269` → `vm.wait_async()`，与此类型无关。

**风险**：无。

**验证**：`cargo build` + `cargo test`（尤其 `bytecode_*` 系列：`bytecode_parity` / `bytecode_async` / `bytecode_default`）全绿。`bytecode_async.rs` 测试的是 `wait_async` 路径，不依赖被删类型。

---

### A3. 删除 `io_awaitable.rs` + `io_selector/`（4 平台后端）

**删什么**
- `src/object/io_awaitable.rs`（`TcpConnectAwaitable` / `TcpReadAwaitable` / `TcpWriteAwaitable`）。
- `src/object/io_selector.rs`（跨平台抽象）。
- `src/object/io_selector/epoll.rs`、`kqueue.rs`、`poll.rs`、`iocp.rs`（共 ~750 行真实平台代码）。
- `src/object/io_selector_integration_test.rs`（仅测试被删 selector）。
- `src/object/mod.rs`：
  - `:8-9`：`#[cfg(not(feature = "tokio"))] mod io_selector;`
  - `:22`：`pub use io_awaitable::{TcpConnectAwaitable, TcpReadAwaitable, TcpWriteAwaitable};`
  - `:23-24`：`#[cfg(not(feature = "tokio"))] pub use io_selector::{Event, Interest, Token};`

**为什么安全**（证据 C2）
- 三个 `Tcp*Awaitable::new` 零调用；其 `poll()` 在 `WouldBlock` 时永远返回 `Pending`（`io_awaitable.rs:67-71` 注释自承 "In a real implementation, this would register with epoll..."）。
- 所有 stdlib 网络 / GTP transport 直接用 `std::net::TcpStream`，不走这些包装。
- 默认 feature（`tokio`）下 `io_selector` 本就不编译——此删除对默认构建零影响，仅清理 `--no-default-features` 路径的死代码。

**⚠️ 决策点（需你拍板）**
- `io_selector` 的 4 个平台后端是**高质量真实实现**（非桩），未来若要做"无 tokio 的纯原生异步"可能有用。删除是不可逆的（git 可恢复，但需明确）。
- **选项 A（推荐，纯清理）**：全部删除。理由：当前生产路径完全不依赖它们，保留等于维护未接线代码；如未来需要，可从 git 历史恢复。
- **选项 B（保守）**：仅删 `io_awaitable.rs`（确定无用的包装），保留 `io_selector/` + `io_selector.rs`，并在 `object/mod.rs` 顶部加注释说明"保留备用，当前未接入"。

> 本方案默认推荐选项 A。若选 B，A3 的删除范围缩减，A4/A5 中 `EventLoop` 因仍 `register_io`→`Selector` 而**不能删**，需相应调整为"仅删 io_awaitable + 标注 EventLoop 为实验性"。

**风险**：选项 A 无功能风险；唯一损失是 4 个平台后端的源码（git 可追溯）。

**验证**
1. `cargo build`（默认 feature）+ `cargo build --no-default-features`（非 tokio 路径）均通过。
2. `cargo test` 全绿。`io_selector_integration_test.rs` 随之删除，不残留。

---

### A4. 删除 `event_loop.rs` + `timer_wheel.rs`

**删什么**
- `src/object/event_loop.rs`（~390 行，含测试）。
- `src/object/timer_wheel.rs`（~190 行）。
- `src/object/mod.rs`：
  - `:6`：`mod event_loop;`
  - `:11`：`mod timer_wheel;`
  - `:21`：`pub use event_loop::EventLoop;`
  - `:26`：`pub use timer_wheel::{TimerAwaitable, TimerWheel};`

**为什么安全**（证据 C3、C4）
- `EventLoop::new()` 仅出现在自身测试、`io_selector_integration_test.rs`（A3 已删）、`bytecode/awaitable.rs:99`（A2 已删）、`native.rs:20,37`（doc 注释）、`native.rs:71`（`create_runtime()`，仅自身测试调用）。
- `TimerWheel` 唯一非自身调用者是 `EventLoop`（`event_loop.rs:51,298,64`），随 EventLoop 一并消失。
- `TimerAwaitable` 零调用。
- 生产异步路径是 `VirtualMachine::wait_async()` + `AsyncCompletionQueue`，与 EventLoop 无关。

**依赖前置**：必须在 **A2、A3 之后**执行（否则 `event_loop.rs` 引用的 `bytecode/awaitable`、`io_selector`、`io_awaitable` 会断链）。实际上 A4 删 EventLoop 后，反过来也消除了 A3 中 selector 的唯一"潜在"消费者。

**风险**：无（删除纯死代码）。`@std/timers` 模块（`stdlib/modules/timers.rs`）需确认**不**依赖 `TimerWheel`——快速核查：`timers.rs` 通过 `vm.next_timer_id()` + 完成队列实现，不走 `TimerWheel`（待实施时二次确认，见 §5 验证清单 V-Timers）。

**验证**
1. `cargo build` + `cargo test`。
2. 专项：跑含 `@std/timers`（setTimeout/setInterval）的 fixture / 测试，确认定时器功能不受影响（V-Timers）。

---

### A5. 删除 `awaitable.rs`（trait / PollResult / Waker / WakerRegistry）

**删什么**
- 文件 `src/object/awaitable.rs`（~110 行）。
- `src/object/mod.rs`：
  - `:4`：`mod awaitable;`
  - `:19`：`pub use awaitable::{Awaitable, PollResult, Waker, WakerRegistry};`

**⚠️ 关键依赖处理（必须先做）**
`Promise`（`src/object/promise.rs`）当前依赖此模块：
- `promise.rs:22` 字段 `wakers: WakerRegistry`
- `promise.rs:39,64` `WakerRegistry::new()`
- `promise.rs` 实现了 `impl Awaitable for Promise` + 一个 `self.poll(...)` 自递归（`:139`）

但生产路径**不 poll Promise**——Promise 的 settle/continuation 通过完成队列在 VM 线程解析。因此需先**改造 `Promise`**，剥离 `Awaitable` impl 与 `WakerRegistry` 字段：

1. 删除 `promise.rs` 中的 `impl Awaitable for Promise` 整块（含 `:139` 的自递归 poll）。
2. 删除 `promise.rs:22` 的 `wakers: WakerRegistry` 字段及其构造（`:39,64`）。
3. 移除 `promise.rs` 顶部对 `crate::object::{Awaitable, PollResult, Waker, WakerRegistry}` 的 import。
4. 确认 `Promise` 剩余公共 API（`new` / `resolve` / `reject` / `then` / `state` 等）不依赖上述符号。

**为什么安全**（证据 C5）
- 生产从不调用 `Promise::poll()`；解析发生在 `vm.rs:187-203 drain_async_completions`。
- `WakerRegistry` 唯一消费者是 Promise 本身。
- `PollResult` 外泄点仅 `tokio_rt.rs`（A6 处理）和 `awaitable_bridge.rs`（A1 已删）。

**风险**：中（涉及改造仍在线的 `Promise`）。
- 需仔细检查 `promise.rs` 是否有除 `impl Awaitable` 外的 `poll`/`waker` 使用（如 `then` 链是否注册 waker）。
- **回滚成本低**：若 Promise 改造引入问题，可仅回滚 A5，保留前序步骤。

**验证**
1. `cargo build` 通过（这是最强的类型一致性检查——任何遗漏的 `Awaitable`/`Waker` 引用都会编译失败）。
2. `cargo test`，尤其 `bytecode_async.rs`、`async_completion.rs`、含 `@std/async` / `@std/http` async / `@std/timers` 的测试。

---

### A6. 清理 `Session.tokio_runtime` 残留 + `tokio_rt.rs` 桥接残留

**删什么**
- `src/runtime/mod.rs`：
  - `:54-55` 字段：`#[cfg(feature="tokio")] tokio_runtime: Option<...TokioRuntime>,`
  - `:75-76` 构造：`#[cfg(feature="tokio")] tokio_runtime: Some(TokioRuntime::new()),`
  - `:84-87` `has_tokio()`
  - `:89-93` `tokio_runtime()`
- `src/async_runtime/tokio_rt.rs` 中依赖已删 `PollResult` 的部分：
  - `BridgedResult` / `from_poll_result`（`:26,94,110-115` 等，需 grep 确认无外部调用后删除）。
- `src/async_runtime/native.rs`（整个文件，~84 行）：
  - 仅含 `pub type NativeRuntime = EventLoop`（指向已删的 EventLoop）+ `create_runtime()`（A4 后断链）+ doc 注释。
  - `src/async_runtime/mod.rs:38 pub mod native;`、`:51 pub use native::NativeRuntime;`

**⚠️ `tokio_rt.rs` 不能整删**
- `tokio_rt.rs` 的 `TokioRuntime`（`spawn` / `block_on` / worker 线程）虽不再被 Session 持有，但 `net_http_client.rs:77-92` 是否复用它？**证据 C9 表明 HTTP 客户端自建独立 runtime**（`tokio::runtime::Builder::new_multi_thread()`），**不**用 `TokioRuntime`。因此：
  - 若确认 `TokioRuntime` 全树无其他调用者 → 可连同 `tokio_rt.rs` 整文件删除（最干净）。
  - 若仍有调用者（如 example）→ 仅删 `BridgedResult`，保留 `TokioRuntime` 主体。
- 实施时先 grep `TokioRuntime` 全树再定夺。

**可选附带清理**
- `examples/tokio_demo.rs`、`examples/awaitable_bridge_demo.rs`：若引用已删符号则一并删除或更新。

**为什么安全**（证据 C7、C9）
- `Session::tokio_runtime()` 零调用；`has_tokio()` 仅 example。
- HTTP 客户端用自带 runtime，不读 Session 的 tokio 句柄。
- 删除后默认构建中 tokio runtime 的构造点从"Session + HTTP 各一个"减为"仅 HTTP 一个"——**顺便修复了"默认构建产生两个 tokio runtime"的问题**。

**风险**：低-中。
- 主要风险是漏掉某个 `#[cfg(feature="tokio")]` 分支对 `TokioRuntime` 的引用 → 编译会立即报错，易定位。
- `--no-default-features`（非 tokio）路径需单独编译验证。

**验证**
1. `cargo build` + `cargo build --no-default-features`。
2. `cargo test` + `cargo test --no-default-features`。
3. HTTP async 路径冒烟：跑 `tests/stdlib_p8_http.rs`（HTTP 客户端异步）确认行为不变。

---

### 2.7. 推荐执行顺序（强约束）

依赖关系决定了顺序。**每步单独提交，便于回滚**：

```
A1 (awaitable_bridge)        ← 删 PollResult 的外泄点之一
  → A2 (bytecode/awaitable)  ← 删 EventLoop 的调用者之一
    → A3 (io_awaitable + io_selector)  ← 选项 A：删 selector 及其包装
      → A4 (event_loop + timer_wheel)   ← 依赖 A2、A3 先行
        → A5 (awaitable trait)           ← 须先改造 Promise
          → A6 (Session.tokio + native.rs + tokio_rt 残留)
```

每步后：`cargo build`（默认 + `--no-default-features`）+ `cargo test`。

---

## 3. P0-B：拆分 VM God Object（HTTP/stream 转换逻辑外移 + 去重）

### 问题定位

`VirtualMachine`（`src/object/vm.rs`，469 行）在核心职责之外，承担了 ~220 行 HTTP/stream 响应→Object 转换（`vm.rs:252-469`），且这部分逻辑与 `stdlib/modules/stream.rs`、`net_http_client.rs` **高度重复**（证据 C8、C9、C10）。

重复对照（**关键**）：

| `vm.rs`（待处理） | `stdlib/modules/` 已有等价物 | 差异 |
|---|---|---|
| `CompletionStreamState {text,pos,closed}` (`:313`) | `stream.rs:92 StreamState` | 字段完全相同 |
| `completion_stream_from_text` (`:319`) | `stream.rs:49 stream_from_text` + `:177 stream_from_text_object` | 构建逻辑相同 |
| `completion_stream_read` (`:403`) | `stream.rs:98 stream_read` | 相同 |
| `completion_stream_read_text` (`:424`) | `stream.rs:123 stream_read_text` | 相同 |
| `completion_stream_read_line` (`:443`) | `stream.rs:148 stream_read_line` | 相同 |
| `completion_stream_read_all` (`:463`) | `stream.rs:168 stream_read_all` | **轻微差异**：vm 版 closed 时返回空串；stream 版返回剩余切片（见 V-ReadAll） |
| `async_http_response_to_object` (`:266`) | `net_http_client.rs:440`（同名私有 fn） | 仅 `ok` 判定写法不同：`(200..300).contains` vs `>=200 && <300`（等价） |
| `async_http_stream_response_to_object` (`:285`) | `net_http_client.rs:531 build_http_stream_response` | 结构相同 |
| `async_completion_data_to_object` (`:252`) | 无等价（`AsyncCompletionData` 枚举派发，唯一独有逻辑） | — |

### 方案：去重 + 外移（而非简单搬迁）

**核心思路**：`vm.rs` 的私有副本是重复方，应**删除副本并复用 stdlib 已有 `pub(crate)` 函数**。但这引入一个**层级倒置**问题：`object/vm.rs`（底层）将 `use crate::stdlib::modules::stream::stream_from_text_object`（上层），违反"object 不依赖 stdlib"的分层。

**两个候选方案，需你选其一**：

#### 方案 B-1（推荐）：下沉共享逻辑到 object 层

1. **新建** `src/object/http_stream.rs`（或复用命名 `completion.rs`，但避免与 `async_runtime/completion.rs` 混淆），将以下逻辑定为 `pub(crate)` 并置于 object 层（底层、无 stdlib 依赖）：
   - `StreamState`（从 `stream.rs` 迁移，改名 `CompletionStreamState` 或保留）
   - `stream_from_text_object` / `stream_read` / `stream_read_text` / `stream_read_line` / `stream_read_all`
   - `http_response_to_object(response: AsyncHttpResponse)` / `http_stream_response_to_object(...)`
   - `async_completion_data_to_object(data: AsyncCompletionData)`（唯一独有逻辑，留这里）
2. `vm.rs`：删除 `:252-469` 全部，`drain_async_completions:193` 改调 `crate::object::http_stream::async_completion_data_to_object`。
3. `stdlib/modules/stream.rs`：改为 `use crate::object::http_stream::*`，删除自身重复定义，仅保留 `stream_module()` / `stream_from_string` 这层薄壳。
4. `stdlib/modules/net_http_client.rs`：`build_http_response` / `build_http_stream_response` 改为调用 object 层共享函数（或保留其 ureq-specific 入口、内部委托共享构造）。
5. `src/object/mod.rs`：`pub(crate) mod http_stream;`（不对外 pub）。

**优点**：彻底消除三处重复；分层正确（共享逻辑在底层）；后续 HTTP/stream 逻辑有单一真源。
**代价**：动 3 个文件 + 新建 1 个；需仔细处理 `stream.rs` / `net_http_client.rs` 的 import 调整。

#### 方案 B-2（最小改动）：仅搬 `async_completion_data_to_object`，复用 stdlib

1. 仅保留 `async_completion_data_to_object`（独有逻辑）——搬到一个小的 object 子模块（如 `src/object/async_completion.rs`）。
2. 删除 `vm.rs` 的 `async_http_response_to_object` / `async_http_stream_response_to_object` / 全部 `completion_stream_*` / `CompletionStreamState`。
3. 在 `async_completion_data_to_object` 的 HTTP 分支里，调用 `crate::stdlib::modules::stream::stream_from_text_object` 和 net_http_client 的 builder。

**优点**：改动最小，`vm.rs` 直接瘦身 ~200 行。
**缺点**：**违反分层**（object → stdlib 依赖倒置）；重复仅在 vm 侧消除，stream.rs / net_http_client.rs 之间的重复仍在。

> **推荐 B-1**：既然要拆，一次性消除重复更值得；分层倒置是技术债。若你希望最小风险、快速见效，可选 B-2 并把分层修正留作后续。

### vm.rs 的瘦身效果

无论 B-1 / B-2，`VirtualMachine` 结构体本身不变（字段都属核心职责），但 `vm.rs` 文件从 **469 行降至 ~250 行**，`VirtualMachine` 的 `impl` 块边界清晰停留在 `:245`（`wait_async` 结束），其后不再有游离的 HTTP/stream 逻辑。

### 唯一独有函数 `async_completion_data_to_object` 的归宿

它是 `AsyncCompletionData` 枚举（`Undefined`/`Text`/`JsonText`/`Bytes`/`HttpResponse`/`HttpStreamResponse`）的派发器，唯一调用点 `drain_async_completions:193`。它**逻辑上属于"完成数据 → Object"的边界转换**，与 HTTP 强相关但不仅限 HTTP（`Text`/`Bytes` 等非 HTTP 分支）。放在 object 层的 http_stream / async_completion 子模块都合理。

### 依赖（新模块的 `use`）

搬出的代码仅依赖（已在 `object/mod.rs:27-31` re-export）：
```rust
use super::value::{bool_obj, num_obj, str_obj, new_error, ArrayData, HashData, Builtin, CallContext, Object};
use crate::async_runtime::{AsyncCompletionData, AsyncHttpResponse};
use std::cell::RefCell;
use std::rc::Rc;
```
（`new_error`/`CallContext` 仅 `completion_stream_size` 用；若改用 stream.rs 的内联校验可省去。）

### ⚠️ 行为等价性注意点（必须验证）

1. **`read_all` 差异（V-ReadAll）**：vm 版在 `closed || pos>=len` 时返回**空串**；stream 版总是返回 `s.text[pos..]`（即便已读完返回空串，但 closed 状态下仍返回剩余内容而非空）。需确认 HTTP stream body 的 `readAll` 在 close 后的行为契约，**以现有 vm 版行为为准**或统一为更合理的一者。建议：统一为"closed 或读完返回空串"，并在 stream.rs 注释。
2. **`ok` 判定**：`(200..300).contains(&status)` 与 `status>=200 && status<300` 数值等价，无行为差异。
3. **Builtin 名前缀**：vm 版用 `http.streamAsync.body.read`；stream.rs 用 `stream.read`。这些是 inspect/debug 时的函数名，对 `Object` 相等性与执行无影响，但**影响错误信息可读性**。B-1 下统一为一套命名（建议保留各调用点的语义前缀，或参数化）。

### 验证

1. `cargo build` + `cargo test`。
2. **专项冒烟**：
   - `tests/stdlib_p8_http.rs`（HTTP 客户端，含 stream 响应）。
   - `tests/bytecode_async.rs`（async drain 路径）。
   - 任何涉及 `@std/stream`、`@std/http` streamAsync body.read/readText/readLine/readAll 的脚本。
3. **等价性核对（V-Equiv）**：对比重构前后，对同一 HTTP 响应，body stream 的 read/readText/readLine/readAll/close 输出逐项一致（可用一个小 .gs fixture）。

---

## 4. 风险矩阵总览

| 步骤 | 风险等级 | 主要风险 | 回滚成本 |
|---|---|---|---|
| A1 | 极低 | 无 | 单文件回滚 |
| A2 | 极低 | 无 | 单文件回滚 |
| A3（选 A） | 低 | 丢失高质量平台后端源码（git 可恢复） | git revert |
| A4 | 低 | `@std/timers` 隐性依赖（待 V-Timers 确认） | 需连同 A2/A3 回滚 |
| A5 | **中** | Promise 改造遗漏 waker/poll 引用 | 回滚 A5（保留前序） |
| A6 | 低-中 | 漏掉 `#[cfg(feature="tokio")]` 分支引用（编译即报） | 回滚 A6 |
| B-1 | 中 | 三文件 import 调整 + read_all 行为统一 | 回滚 B-1 |
| B-2 | 低 | 分层倒置（技术债留存） | 回滚 B-2 |

---

## 5. 实施前/后验证清单（Checklist）

### 编译矩阵（每步后）
- [ ] **VC1** `cargo build`（默认 features = tokio）
- [ ] **VC2** `cargo build --no-default-features`（无 tokio 路径）
- [ ] **VC3** `cargo build --all-features`

### 测试矩阵（每步后）
- [ ] **VT1** `cargo test`
- [ ] **VT2** `cargo test --no-default-features`
- [ ] **VT3** `cargo test --test bytecode_parity`（VM↔treewalker 一致性，75 fixture）
- [ ] **VT4** `cargo test --test bytecode_async`（async drain 路径）

### 专项（标注步骤）
- [ ] **V-Timers**（A4 后）：`@std/timers` setTimeout/setInterval 行为正常
- [ ] **V-HTTP**（A6、B 后）：`tests/stdlib_p8_http.rs` 全绿
- [ ] **V-ReadAll**（B 后）：stream body readAll 在 closed 状态行为符合预期
- [ ] **V-Equiv**（B 后）：重构前后 HTTP stream body 各 read 方法输出逐项一致

### 公共 API 稳定性（A 全部完成后）
- [ ] **VAPI1** `gts::object` 公开 re-export 中被删符号（`EventLoop`/`TimerWheel`/`Awaitable`/`PollResult`/`Tcp*Awaitable` 等）——**确认这些是否属于承诺的公共 API**。若 `gts` 作为库被外部消费（如 `examples/`、未来 SDK），需评估破坏性。当前证据：仅 `examples/` 与自身测试使用，无外部消费者。
- [ ] **VAPI2** `gts::async_runtime` 公开 re-export（`NativeRuntime`/`TokioRuntime`/`spawn_blocking_gts` 等）同上评估。

---

## 6. 预期收益（量化）

| 指标 | 现状 | 重构后 |
|---|---|---|
| 异步栈抽象层数 | 7（3 在用） | 3（全部在用） |
| 删除死代码行数 | — | **~2,000+ 行**（A1-A6） |
| `vm.rs` 行数 | 469 | ~250（B-1/B-2） |
| 默认构建 tokio runtime 构造数 | 2（Session + HTTP 各一） | 1（仅 HTTP） |
| stream/HTTP 响应构造逻辑副本数 | 3（vm + stream.rs + net_http_client.rs） | 1（B-1）或 2（B-2） |
| `VirtualMachine` 职责数 | 4（含 HTTP/stream 转换） | 3（配置/globals/异步协调） |

---

## 7. 决策点汇总（需你确认）

1. **A3 选项**：io_selector 4 平台后端——**选 A（全删，推荐）** 还是 **选 B（仅删 io_awaitable，保留 selector 备用）**？
2. **B 方案**：**B-1（下沉共享到 object 层，推荐，彻底去重）** 还是 **B-2（最小改动，接受分层倒置）**？
3. **公共 API 破坏**：被删符号（`EventLoop`/`Awaitable`/`PollResult` 等）是否属于承诺的公共 API？若 `gts` 仅内部消费（CLI + 自带测试/example），则可视为非破坏性。（证据指向：是。）
4. **examples 处理**：`examples/awaitable_bridge_demo.rs`、`examples/tokio_demo.rs` 是否一并删除？（推荐：删除引用已删符号者。）

---

## 8. 不在本方案范围（明确排除）

- 树遍历器下线（P3，需独立 roadmap）。
- 标准库样板清理（P1，独立低风险任务，可单独脚本化）。
- 小 bug 修复（P2：value.rs 重复 arm、expressions.rs:186 unwrap、environment.rs borrow、bytecode/mod.rs 过时注释）——可与本方案合并到一个 PR，也可独立。

---

*审阅后，请告知：①A3 选 A/B；②B 选 B-1/B-2；③是否确认公共 API 非破坏；④是否授权按 §2.7 顺序逐步实施（或仅出文档、暂不动手）。*
