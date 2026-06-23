# Tokio 单 Worker 并发改造方案

> 目标：让 `@std/web` 在单 worker 下也能处理并发请求，尤其是 `gs-llm-bridge`
> 这类代理服务在等待上游 HTTP I/O 时，不阻塞整个 worker。

## 设计原则

- **不兼容式更新优先**：当前 `gts_r` 尚未正式发布稳定版本，可以直接进行不兼容式修改。实现上优先删除旧的同步双路径和兼容包袱，避免为了平滑迁移保留两套 Web/HTTP/Promise 机制，从而减少迭代造成的代码冗余。
- **VM 单线程，I/O 交给 Tokio**：GoScript 对象模型基于 `Rc<RefCell>`，不跨线程传递 `Object`。脚本执行、对象读写、Promise resolve 仍在 VM 所在线程完成；Tokio 只负责 socket、HTTP、timer 等可 `Send` 的 I/O 工作。
- **异步是默认 Web 语义**：`app.listen(port)` 的单 worker 模式应从同步串行 loop 变成事件循环驱动。需要定额测试时保留测试专用选项，但运行时默认不再以“一次请求处理到底”为核心模型。
- **连接池是代理性能的一部分**：代理链路必须默认使用 keep-alive 和连接复用，否则单 worker 即使能并发，也会在 Windows 上继续触发 `os error 10048`。
- **先打通非流式，再打通流式**：`requestAsync` 先稳定承载普通 JSON 代理；`streamAsync` 和 SSE 转发放到第二阶段，避免一次性改动过大。

## 当前阻塞点

当前 Web 请求路径是同步串行的：

```text
tiny_http recv
  -> web_handle_request
  -> 执行 handler
  -> http.request/http.stream 阻塞等待上游
  -> request.respond
  -> 回到 recv
```

这意味着单 worker 下，一个慢上游请求会占住整个 worker。压测中 `chat same-protocol c=1/c=10` 正常，但高并发阶段出现大量 `os error 10048`，说明当前同步 HTTP 客户端还存在连接复用不足的问题。

## 目标架构

```text
Web accept loop
  -> 创建 RequestTask
  -> 调用 JS handler
      -> 若同步完成：立即 respond
      -> 若返回 Promise：挂起 response handle
  -> 继续 accept 下一请求

Tokio runtime
  -> 执行 HTTP/TCP/timer I/O
  -> 通过 completion queue 发送纯数据结果

VM event loop
  -> drain completion queue
  -> resolve/reject Promise
  -> 恢复对应 handler
  -> 完成 response
```

跨线程只允许传递这些数据：

- `String`
- `Vec<u8>`
- status code
- headers 的 owned map
- JSON 文本或已序列化 payload
- 错误字符串

禁止跨线程传递：

- `Object`
- `Rc<...>`
- `EnvRef`
- `CallContext`
- `tiny_http::Request` 中与 VM 对象混杂的状态

## 阶段计划

### 阶段 1：运行时入口统一到 Tokio 能力

- 将 release 构建默认启用 `tokio` feature，或移除 feature gate，直接把 Tokio 作为默认 runtime 依赖。
- `Session::new()` 直接具备 Tokio I/O 能力；不再保留 `Session::with_tokio()` 公开构造入口。
- CLI 增加可观测输出：运行模式显示 `bytecode + tokio-io`。
- 删除或收敛“native async runtime”与“tokio async runtime”的重复公开入口，保留一个 runtime façade。

验收：

- `cargo test --release` 通过。
- `gs -v` 或 runtime state 能显示 Tokio I/O 已启用。

### 阶段 2：Completion Queue 与 Promise 恢复

- 在 VM 或 Session 层增加 async completion queue。
- Tokio task 完成后通过 channel 发送 completion。
- VM event loop 增加 `poll_async_completions()`。
- Promise resolve/reject 必须发生在 VM 线程。
- `await` / Promise callback 恢复后继续执行 bytecode frame。

验收：

- 新增 `@std/async` 或内部测试：一个 Promise 由 Tokio timer 完成，JS 侧 `await` 能继续执行。
- 并发发起多个 timer/HTTP mock，不互相阻塞。

### 阶段 3：异步 HTTP Client

- 新增 `@std/http.requestAsync(options)`。
- 内部使用 Tokio 异步 HTTP 客户端，建议选型：
  - `reqwest`：实现快，连接池成熟。
  - `hyper`：控制更细，但落地成本更高。
- 默认启用 keep-alive、连接池、连接超时、请求超时。
- 返回 Promise，resolve 为 `{ status, headers, body }`。
- 将同步 `http.request` 改为 `requestAsync` 的阻塞包装或直接移除同步实现。考虑不兼容式更新：优先让核心代码只维护异步实现。

验收：

- `http.requestAsync` 能并发请求本地 mock upstream。
- Windows 下并发 50 不再快速触发 `10048`。

### 阶段 4：Web Handler Promise 化

- `@std/web` handler 返回 Promise 时，不立即构造 response。
- response state 拆成可挂起的 `PendingResponse`。
- `res.send/json/status/setHeader` 仍只能在 VM 线程调用。
- 请求级任务完成后统一 finalize response。
- 单 worker accept loop 不等待某个 handler 的上游 I/O 完成。

建议不兼容调整：

- Express 风格 handler 统一为 `(req, res, next)`，`ctx` 风格可以删除或降级为兼容测试用，避免长期维护双签名分支。
- `web.json()` 保留为默认 body parser，但实现应适配 async request task。
- `app.listen(port)` 默认就是 async single-worker；`{count}` 仅测试使用。

验收：

- 单 worker 下两个请求：一个上游 sleep 2s，一个 `/healthz`，`/healthz` 不被阻塞。
- `gs-llm-bridge` 管理接口与代理接口均通过 smoke。

### 阶段 5：gs-llm-bridge 代理链路切换

- 将 `proxy_service.gs` 的上游调用从 `http.request` 切到 `await http.requestAsync`。
- 管理接口保持同步风格即可，但 route handler 允许 async。
- 代理链路错误处理统一 await 结果。
- 压测默认使用单 worker，验证请求排队是否消失。

验收：

- `chat same-protocol c=1,10,50` fail=0。
- c=50 p95 不再出现秒级排队。
- 不再出现本机端口耗尽作为主要失败。

### 阶段 6：流式代理与 SSE

- 新增 `http.streamAsync(options)`。
- 返回 async reader，或提供 callback/iterator 风格：
  - `for await (let chunk of response.body) { ... }`
  - 或 `response.body.readAsync()`
- Web response 支持 chunked/SSE 写出。
- 上游流读取与下游写出都不能阻塞 accept loop。

验收：

- `chat stream c=1,10` fail=0。
- SSE content-type 与 chunk 格式正确。
- 长连接流式请求存在时，普通 `/healthz` 不被阻塞。

## 删除与收敛清单

为避免不兼容式迭代期间积累冗余，以下内容在新实现稳定后应直接删除或收敛：

- 同步 `ureq` HTTP 主路径。
- Web handler 的长期双模型分支。
- native/tokio 两套公开 async runtime API。
- 只为旧同步语义存在的 `web_listen_serial` 默认路径。
- 不能支持真实异步恢复的 Promise 占位实现。

## 风险

- Tokio 多线程不能直接执行 GTS VM；必须严格保持 `Object` 不跨线程。
- `tiny_http::Request` 是否适合延迟 respond 需要验证；如果不适合，应替换 Web server 底层为 hyper/axum 风格的 Tokio server。
- `await` 恢复 bytecode frame 的能力必须完整，否则 Web handler Promise 化会卡在语言层。
- 流式代理涉及 backpressure，需要避免无限缓存上游数据。

## 推荐最终形态

- `gts_r` 默认运行模式：`bytecode + tokio I/O`。
- `@std/web` 默认支持单 worker 并发。
- `@std/http` 默认异步、连接池化。
- 同步 API 若保留，只作为异步 API 的薄包装，不再维护独立实现。
- `gs-llm-bridge` 默认单 worker 即可承载并发代理；多 worker 只用于 CPU 密集或隔离需求。
