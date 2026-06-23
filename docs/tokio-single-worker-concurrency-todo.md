# Tokio 单 Worker 并发开发 TODO 清单

> **文档用途**：本文件是 Tokio 单 worker 并发改造的执行追踪表。每完成一项必须更新状态、填写证据并提交代码。
>
> **对应方案**：`docs/tokio-single-worker-concurrency-plan.md`
>
> **核心策略**：当前 `gts_r` 尚未正式发布稳定版本，允许直接进行不兼容式修改。实现上优先收敛旧同步路径，避免为了兼容过渡保留重复架构。

## 状态说明

- `[ ]` 待办
- `[~]` 进行中
- `[x]` 完成
- `[!]` 阻断

每条完成证据必须写具体文件、测试、命令或压测结果，禁止只写“完成”。

---

## 阶段 0：基线与约束

- [x] T0.1 记录压测现状与主要瓶颈
  - 证据：`gs-llm-bridge` bytecode 标准压测：`/healthz ~35k rps`，`chat same-protocol c=1/c=10 fail=0`；高并发出现 Windows `os error 10048`，说明出站连接复用不足。
- [x] T0.2 明确不兼容式更新策略
  - 证据：`docs/tokio-single-worker-concurrency-plan.md` 已写入“不兼容式更新优先”原则。
- [x] T0.3 建立单 worker 并发验收 fixture
  - 证据：`tests/stdlib_p9_web.rs` 新增 ignored 测试 `web_single_worker_does_not_block_fast_route_while_slow_route_waits`；`cargo test --test stdlib_p9_web --release` 为 11 passed / 1 ignored。

## 阶段 1：Runtime 入口统一到 Tokio I/O

- [x] T1.1 默认构建启用 Tokio 能力
  - 证据：`Cargo.toml` 默认 feature 调整为 `["tokio"]`；`cargo test --release --test stdlib_p9_runtime` 通过。
- [x] T1.2 `Session::new()` 具备 Tokio I/O runtime
  - 证据：`src/runtime/mod.rs` 中 `Session::new()` 默认创建 `TokioRuntime`。
- [x] T1.3 CLI/runtime state 暴露运行模式
  - 证据：`src/bin/gs.rs` 的 `gs -v` 输出 `bytecode + tokio-io`；`src/stdlib/modules/runtime.rs` 暴露 `@std/runtime.mode` 和 `runtime.state()`。
- [x] T1.4 删除或收敛 native/tokio 双公开入口
  - 证据：删除 `src/runtime/mod.rs` 的 `Session::with_tokio()` 公开构造入口；`examples/tokio_demo.rs` 与 Tokio 示例文档统一使用 `Session::new()`；验证：`rustfmt --check src/runtime/mod.rs examples/tokio_demo.rs src/async_runtime/awaitable_bridge.rs`、`cargo test --lib runtime --features tokio`、`cargo test --example tokio_demo --features tokio`、`cargo test --test bytecode_default --features tokio` 通过。

## 阶段 2：Async Completion Queue

- [x] T2.1 增加线程安全 completion 数据结构
  - 证据：新增 `src/async_runtime/completion.rs`，提供 `AsyncCompletion` / `AsyncCompletionQueue` / `AsyncCompletionSender`，跨线程只传 owned `Send` 数据。
- [x] T2.2 VM 线程 drain completion 并 resolve/reject Promise
  - 证据：`src/object/vm.rs` 新增 `completion_id -> Promise` 登记表，`drain_async_completions()` 在 VM 线程把 owned completion 转成 `Object` 并 resolve/reject；`tests/async_completion.rs` 覆盖 resolve/reject Promise，`cargo test --release --test async_completion` 通过。
- [x] T2.3 Tokio task 能把结果投递回 VM
  - 证据：`tests/async_completion.rs` 覆盖后台线程与 Tokio task 投递 completion，`cargo test --release --test async_completion` 通过。
- [x] T2.4 `wait_async` 改为事件循环式 drain
  - 证据：`src/async_runtime/completion.rs` 的 completion queue 增加 `Condvar` 通知；`src/object/vm.rs` 的 `wait_async()` 在 registered Promise 存在时等待 completion 通知并 drain，不再单纯 sleep polling；`tests/async_completion.rs` 新增 `wait_async_wakes_when_registered_completion_arrives`，`cargo test --release --test async_completion` 通过。

## 阶段 3：异步 HTTP Client

- [x] T3.1 新增 `@std/http.requestAsync(options)`
  - 证据：`src/stdlib/modules/net_http_client.rs` 新增 `requestAsync`，返回 Promise 并通过 VM completion queue resolve `{ status, statusText, headers, body, ok }`；`src/stdlib/mod.rs` 新增 `@std/http` alias；`src/bytecode/interp.rs` 与 `src/evaluator/expressions.rs` 在 `await` pending Promise 时触发 `vm.wait_async()` drain completion；`tests/stdlib_p8_http.rs` 覆盖本地 mock HTTP async 请求；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p8_http http_client_request_async_returns_promise_response`、`cargo test --release --test async_completion`、`cargo test --release --test bytecode_async` 通过。
- [x] T3.2 引入连接池 HTTP 客户端
  - 证据：`Cargo.toml` 在 tokio feature 下新增 `reqwest`；`src/stdlib/modules/net_http_client.rs` 的 `requestAsync` 切到全局复用 Tokio runtime + reqwest client，启用 keep-alive 连接池；`tests/stdlib_p8_http.rs` 新增本地 keep-alive fixture，64 次 `await http.requestAsync` 只允许服务端接受一条 TCP 连接，用于验证顺序请求复用连接并减少 Windows 端口 churn；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p8_http http_client_request_async -- --nocapture` 通过。
- [x] T3.3 收敛同步 `http.request`
  - 证据：`src/stdlib/modules/net_http_client.rs` 的 `http.request`/`http.fetch` 改为同一套全局 reqwest client 的阻塞薄包装，默认 tokio feature 下不再走旧 `ureq` request 主路径；`tests/stdlib_p8_http.rs` 新增 `http_client_request_uses_pooled_tokio_client`，16 次同步 `http.request` 只允许本地服务端接受一条 TCP 连接；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p8_http http_client_request_uses_pooled_tokio_client -- --nocapture` 通过。
- [x] T3.4 HTTP async 并发测试
  - 证据：`tests/stdlib_p8_http.rs` 新增 `http_client_request_async_handles_concurrency`，32 个并发 `await http.requestAsync` 本地 mock upstream 全部成功，脚本汇总 `fail=0`；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p8_http http_client_request_async_handles_concurrency -- --nocapture` 通过。

## 阶段 4：Web Handler Promise 化

- [x] T4.1 `@std/web` 支持 handler 返回 Promise
  - 证据：`src/stdlib/modules/web.rs` 在 handler 返回 Promise 时挂接响应 continuation，并在 reject 时返回 500；`tests/stdlib_p9_web.rs` 新增 `web_handler_returned_promise_delays_response_until_settled`，handler 直接返回 pending `http.requestAsync` Promise，本地 delayed upstream 未完成前不会发送响应；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p9_web web_handler_returned_promise_delays_response_until_settled -- --nocapture` 通过。
- [x] T4.2 response state 可挂起
  - 证据：`src/object/promise.rs` 支持 Promise settlement continuation，`src/evaluator/builtins.rs` 的 `Promise.then/catch/finally` 对 pending Promise 返回下游 Promise 而不阻塞等待；`tests/stdlib_p9_web.rs` 新增 `web_async_handler_can_update_response_after_resume`，验证 async handler 恢复后 `res.status`、`res.setHeader`、`res.send` 能影响最终响应；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p9_web web_async_handler_can_update_response_after_resume -- --nocapture`、`cargo test --release --test stdlib_p9_web web_handler_returned_promise_delays_response_until_settled -- --nocapture` 通过。
- [x] T4.3 单 worker accept loop 不等待上游 I/O
  - 证据：`src/stdlib/modules/web.rs` 的单 worker accept loop 在请求返回 pending Promise 时挂起响应并继续 accept，同时在 loop tick 中 drain VM completion；`tests/stdlib_p9_web.rs` 启用并改造 `web_single_worker_does_not_block_fast_route_while_slow_route_waits`，慢路由等待 delayed upstream `http.requestAsync(...).then(...)` 时，同一 worker 的 `/healthz` 可在 150ms 内返回；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p9_web web_single_worker_does_not_block_fast_route_while_slow_route_waits -- --nocapture`、`cargo test --release --test stdlib_p9_web web_async_handler_can_update_response_after_resume -- --nocapture`、`cargo test --release --test stdlib_p9_web web_handler_returned_promise_delays_response_until_settled -- --nocapture`、`cargo test --release --test stdlib_p9_web web_listen_default_serves_multiple_requests -- --nocapture`、`cargo test --release --test bytecode_async`、`cargo test --release --test async_completion` 通过。
- [x] T4.4 收敛 handler 签名
  - 证据：`src/stdlib/modules/web.rs` 移除 `web_handler_prefers_express_args` 与旧 `ctx` wrapper 构造，handler dispatch 统一调用 `(req, res, next)`；`web.json()` 与 `web.static()` middleware 同步改为接收 `req`；`tests/stdlib_p9_web.rs` 的 web handler fixture 全部切到 `(req, res)` / `(req, res, next)`；`docs/web-concurrency-design.md` 示例同步为新签名；验证：`cargo fmt --check`、`cargo test --release --test stdlib_p9_web web_handler_returned_promise_delays_response_until_settled -- --nocapture`、`cargo test --release --test stdlib_p9_web -- --test-threads=1` 通过。

## 阶段 5：gs-llm-bridge 切换

- [ ] T5.1 代理非流式请求切到 `await http.requestAsync`
  - 证据：`gs-llm-bridge/src/services/proxy_service.gs` 非流式路径完成切换。
- [ ] T5.2 单 worker 代理压测
  - 证据：`chat same-protocol c=1,10,50 fail=0`。
- [ ] T5.3 排队延迟验收
  - 证据：c=50 p95 不再出现秒级排队。

## 阶段 6：流式代理与 SSE

- [ ] T6.1 新增 `http.streamAsync`
  - 证据：返回 async reader 或 async iterator。
- [ ] T6.2 Web response 支持 chunked/SSE 写出
  - 证据：上游 chunk 到达后可增量写下游。
- [ ] T6.3 流式压测
  - 证据：`chat stream c=1,10 fail=0`，且 `/healthz` 不被长流阻塞。

---

## 当前指针

T5.1 代理非流式请求切到 `await http.requestAsync`。
