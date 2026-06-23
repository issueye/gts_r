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
  - 证据：`src/runtime/mod.rs` 中 `Session::new()` 默认创建 `TokioRuntime`；`Session::with_tokio()` 收敛为 `Session::new()` 别名。
- [x] T1.3 CLI/runtime state 暴露运行模式
  - 证据：`src/bin/gs.rs` 的 `gs -v` 输出 `bytecode + tokio-io`；`src/stdlib/modules/runtime.rs` 暴露 `@std/runtime.mode` 和 `runtime.state()`。
- [ ] T1.4 删除或收敛 native/tokio 双公开入口
  - 证据：公开 API 只保留一个 runtime facade。

## 阶段 2：Async Completion Queue

- [x] T2.1 增加线程安全 completion 数据结构
  - 证据：新增 `src/async_runtime/completion.rs`，提供 `AsyncCompletion` / `AsyncCompletionQueue` / `AsyncCompletionSender`，跨线程只传 owned `Send` 数据。
- [ ] T2.2 VM 线程 drain completion 并 resolve/reject Promise
  - 证据：Promise resolve/reject 只在 VM 线程执行。
- [x] T2.3 Tokio task 能把结果投递回 VM
  - 证据：`tests/async_completion.rs` 覆盖后台线程与 Tokio task 投递 completion，`cargo test --release --test async_completion` 通过。
- [ ] T2.4 `wait_async` 改为事件循环式 drain
  - 证据：不再单纯 sleep 等待 pending counter。

## 阶段 3：异步 HTTP Client

- [ ] T3.1 新增 `@std/http.requestAsync(options)`
  - 证据：返回 Promise，resolve `{ status, statusText, headers, body, ok }`。
- [ ] T3.2 引入连接池 HTTP 客户端
  - 证据：使用 Tokio async HTTP 客户端；默认 keep-alive；本地压测不再快速触发 `10048`。
- [ ] T3.3 收敛同步 `http.request`
  - 证据：同步 API 仅作为异步实现的薄包装，或直接不兼容式移除旧 `ureq` 主路径。
- [ ] T3.4 HTTP async 并发测试
  - 证据：并发请求本地 mock upstream，fail=0。

## 阶段 4：Web Handler Promise 化

- [ ] T4.1 `@std/web` 支持 handler 返回 Promise
  - 证据：Promise 未完成前不发送响应。
- [ ] T4.2 response state 可挂起
  - 证据：`res.send/json/status/setHeader` 能在 async handler 恢复后完成响应。
- [ ] T4.3 单 worker accept loop 不等待上游 I/O
  - 证据：慢请求处理中，第二个请求可立即完成。
- [ ] T4.4 收敛 handler 签名
  - 证据：优先统一 `(req, res, next)`；旧 `ctx` 路径不再作为长期主路径。

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

T1.4 删除或收敛 native/tokio 双公开入口；下一步同时推进 T2.2 Promise resolve/reject 绑定。
