# GTS Rust 重构 — Session 6 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
承接 Session 5，继续推进 `gts_r` 对 `gts` 的功能复刻。本轮聚焦**实时双向通信**，落地 WebSocket（RFC 6455）的客户端和服务端。

### 新增 2 个 `@std/*` 模块

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/net/ws/client` | RFC 6455 握手 + 帧编解码；`connect(url, headers?)` + `send`/`sendText`/`sendBinary`/`recv`/`close` | `tests/stdlib_p9_ws.rs` | 2（+共享） |
| `@std/net/ws/server` | `createServer(port, handler?)` + `acceptOne`/`accept`/`close`；`upgrade` 在同步运行时不可用（明确报错指向 `acceptOne`） | `tests/stdlib_p9_ws.rs` | 4（+共享） |

合计 **6 个新测试**，全部通过。

### 测试结果
- **新增测试**：6 个（ws），全部通过
- **测试总数**：159 → **165**，全绿
- **无回归**：原有 159 个测试全部保持通过

---

## 实现要点与设计决策

### 1. WebSocket 完全自包含（零新依赖）
Go 版 `ws.go` **本身就是手写 RFC 6455 帧编解码**（没有用 gorilla/websocket 之类的外部库），所以 Rust 端可以原样复刻，复用现有内置 helper：
- 握手 accept-key：`base64(sha1(client_key + GUID))` —— 复用 `sha1()` 和 `base64_std_encode()`
- 客户端 nonce：16 字节随机数 —— 复用 `getrandom_inner()`
- TCP 连接：复用 Session 4 的 `resolve_socket_addr()` + `connect_timeout()`

帧编解码（`ws_write_frame` / `ws_read_frame`）严格按 RFC 6455 §5：FIN 位、opcode、变长 payload（7/16/64 位）、客户端掩码（mask key + XOR）、分片重组、Ping→Pong 自动应答、Close→EOF 语义。

### 2. 客户端连接对象 API
连接对象提供 `send`/`sendText`（文本帧）、`sendBinary`（二进制帧）、`recv`（读取下一帧，EOF/Close 返回 null）、`close`。与 Go 版 `newWSConnObject` 完全对齐。

### 3. 服务端的同步事件循环妥协（与 socket server 一致）
Go 版 `ws.createServer` 用 goroutine 跑后台 accept 循环，但 Rust 的 GTS VM 是**纯同步树遍历器，没有事件循环**。沿用 Session 4 的 socket server 模式：
- `createServer(port, handler?)` 绑定非阻塞 listener，存 handler，立即返回
- `acceptOne(handler?)` 同步阻塞：TCP accept → 内联完成 WS 握手 → 调用 handler（handler 可在 createServer 注册或 acceptOne 显式传入）
- 无 pending 连接时返回 WouldBlock 哨兵

### 4. `ws.upgrade` 的诚实降级
Go 版 `ws.upgrade(reqObj)` 需要 HTTP 请求对象上的 `_raw`（`*http.Request`）和 `_writer`（`http.ResponseWriter`），通过 `Hijacker` 接管底层连接。Rust 端没有运行中的 HTTP 服务来提供这种可被劫持的请求/响应对，所以 `ws.upgrade` 返回明确错误，指引脚本改用 `ws.createServer(port, handler).acceptOne()`。这比假装支持更诚实。

### 5. 测试策略：真服务器回环
同步 VM **无法在一个脚本里同时跑 WS 客户端和服务端**——客户端的 `connect` 会阻塞等待握手响应，但服务端的 `acceptOne`（完成服务端握手）在同一脚本里还轮不到执行，会死锁。

解决方案：在 Rust 测试线程里 spawn 一个**一次性 WS echo 服务器**（手写 SHA1+base64+帧编解码，约 150 行），让 GTS 客户端脚本连接它。`ws_client_echo_roundtrip_against_rust_server` 验证了完整的：TCP 连接 → 客户端握手（带掩码的 Sec-WebSocket-Key）→ 服务端握手响应（Sec-WebSocket-Accept）→ 客户端发送文本帧（masked）→ 服务端回显（unmasked）→ 客户端 recv。

---

## 仍待完成（按优先级）

### 🔴 高优先
- `@std/net/http/server` —— 需要 per-request VM / 线程模型（同 socket/ws server 的异步障碍）
- `@std/web` —— express 风格框架，依赖 http/server
- 完整 ES `import/export` 语义（named/default/namespace/re-export）
- package resolver + `.gspkg`（gs pack / bundle / dist）

### 🟡 中优先
- `@std/signal`、`@std/watch` —— 需要事件循环
- `@std/pty` —— 平台差异大
- `--workers` 真实 worker 池
- typechecker + LSP

### 🟢 低优先
- TUI（crossterm+ratatui，人工验收为主）
- CI 流水线、性能基线

---

## 修改的文件清单

### 源代码
- `src/stdlib/mod.rs` —— 新增 2 个模块（ws/client + ws/server，约 450 行：握手、帧编解码、连接对象、服务端 accept），并在 `load_native_module` 注册

### 测试
- `tests/stdlib_p9_ws.rs` —— ws client/server（6 测试，含真实 Rust echo 服务器回环）

### 文档
- `docs/parity-matrix.md` —— 新增 2 个模块行（ws/client、ws/server 均 partial），更新 network 汇总行，更新测试计数
- `docs/session-6-progress-report.md` —— 本文件

---

## 结论

本次会话新增 2 个标准库模块与 6 个测试，把 Rust 端 `@std/*` 模块数从 58 推进到 **60**，测试总数从 159 增加到 **165**，全绿无回归。

本轮的关键贡献是**打通了 WebSocket 实时双向通信**：GTS 脚本现在可以作为 WS 客户端连接任意 RFC 6455 服务端（带掩码帧、握手验证），也可以创建 WS 服务端（内联 acceptOne 模型）。至此，Rust 端的网络通信栈已经相当完整——HTTP 客户端（S3）、HTTP（缺失服务端）、TCP 客户端/服务端（S4）、WS 客户端/服务端（本轮）、exec（S3）、sqlite db（S3）、mail（S4）、sse（S3）。

唯一仍受异步事件循环阻塞的高价值网络模块是 `@std/net/http/server` 和 `@std/web`——它们需要 per-request VM 或线程池，属于架构级工作，待后续会话处理。
