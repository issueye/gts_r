# GTS Rust 重构 — Session 7 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
承接 Session 6，继续推进 `gts_r` 对 `gts` 的功能复刻。本轮攻克了**最后一个高价值网络模块**——HTTP 服务端，让 GTS 脚本现在可以对外提供 HTTP 服务。

### 新增 1 个 `@std/*` 模块

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/net/http/server` | 同步 HTTP 服务端（tiny_http）；`createServer(handler?, port?)` + `acceptOne`/`accept`/`close` | `tests/stdlib_p9_http_server.rs` | 4 |

合计 **4 个新测试**，全部通过。

### 测试结果
- **新增测试**：4 个（http server），全部通过
- **测试总数**：165 → **169**，全绿
- **无回归**：原有 165 个测试全部保持通过

---

## 实现要点与设计决策

### 1. 选型：tiny_http
Go 版用 `net/http.Server` + goroutine 后台循环。Rust 端需要一个**同步、轻依赖**的 HTTP 库来匹配 GTS 单线程 VM 模型。`tiny_http` 完美契合：
- 纯同步 API（`server.recv()` 阻塞等待请求）
- 单 crate 无外部运行时
- 支持请求体读取、自定义响应头、状态码

### 2. 同步 acceptOne 模型（沿用 socket/ws server 一致模式）
Go 版 `createServer(handler, port)` 用 goroutine 跑后台 accept 循环，但 Rust 的 GTS VM 是**纯同步树遍历器，没有事件循环**。沿用 Session 4/6 既定的模式：
- `createServer(handler?, port?)` 绑定 `tiny_http::Server`，存 handler，立即返回（不阻塞、不 spawn）
- `acceptOne(handler?)` 同步阻塞：`server.recv()` 等单个请求 → 构造请求对象 → 同步调用 handler → 用 handler 累积的响应状态构造 `tiny_http::Response` → `request.respond()` → 返回
- handler 可在 createServer 注册，也可在 acceptOne 显式传入
- `close()` drop 掉 server

### 3. 请求/响应对象（对齐 Go 版 shape）
请求对象：`{method, url, path, body, query, headers, remoteAddr}`，其中：
- `query` 是 Hash（query string 百分号解码后的 key→value）
- `headers` 是 Hash（每个 header 名取首个值）
- `body` 是完整请求体字符串

响应对象：`{status, setHeader, send, json, end}`，通过闭包捕获的 `HttpResponseState` 累积状态：
- `send(text)` 默认 `Content-Type: text/plain`
- `json(obj)` 自动 `Content-Type: application/json` + 内联 JSON 序列化（`hash_to_json`/`value_to_json`/`json_escape_string`，零额外依赖）
- `status(code)` 设置状态码
- `setHeader(k, v)` 设置响应头（Content-Type 走专门分支）
- handler 抛运行时错误时自动返回 500

### 4. 测试策略：子进程服务端 + 测试线程客户端
同步 VM **无法在一个脚本里同时跑 HTTP 服务端和客户端**（服务端的 `acceptOne` 阻塞，客户端没有机会发请求）。沿用 WS 测试的成功模式：
- 把 GTS 服务端脚本作为**子进程**启动
- 服务端脚本 `println("GTS_PORT=" + port)` 把绑定端口吐到 stdout
- Rust 测试线程读 stdout 拿到端口，再用裸 `TcpStream` 发原始 HTTP 请求
- 4 类测试覆盖：GET+text 响应、JSON+自定义状态码、query+body 解析、自定义响应头

### 5. 绑定地址修正
`0.0.1:0` 不是有效地址（Windows 报 os error 10049），改用 `0.0.0.0:0` 让 OS 分配临时端口。

---

## 网络通信栈当前状态

至此 Rust 端的网络通信栈已**基本完整**：

| 能力 | 模块 | 状态 |
|---|---|---|
| HTTP 客户端 | `@std/net/http/client` | compatible (S3) |
| HTTP 服务端 | `@std/net/http/server` | **partial (本轮)** |
| TCP 客户端 | `@std/net/socket/client` | partial (S4) |
| TCP 服务端 | `@std/net/socket/server` | partial (S4) |
| WebSocket 客户端 | `@std/net/ws/client` | partial (S6) |
| WebSocket 服务端 | `@std/net/ws/server` | partial (S6) |
| IP 解析 | `@std/net/ip` | compatible |
| 进程执行 | `@std/exec` | compatible (S3) |
| SQLite | `@std/db` | compatible (S3) |
| SSE 解析 | `@std/sse` | compatible (S3) |
| 邮件解析 | `@std/mail` | compatible (S4) |
| 子脚本执行 | `@std/runtime` | compatible (S5) |

所有网络服务端模块（socket/ws/http server）都受同一个约束：**同步 VM 无事件循环，acceptOne 是单次同步模型而非并发后台循环**。这是 parity matrix 标记 `partial` 而非 `compatible` 的统一原因。完整并发需要 roadmap 阶段 4 引入事件循环。

---

## 仍待完成（按优先级）

### 🔴 高优先
- `@std/web` —— express 风格框架，依赖 http/server（现在基础已就绪，可以叠加路由层）
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
- `Cargo.toml` —— 新增 `tiny_http = "0.12"` 依赖
- `src/stdlib/mod.rs` —— 新增 `@std/net/http/server` 模块（约 330 行：createServer、acceptOne、请求/响应对象、百分号解码、内联 JSON 序列化），并在 `load_native_module` 注册

### 测试
- `tests/stdlib_p9_http_server.rs` —— http server（4 测试，含子进程服务端+测试线程客户端的真实 HTTP 回环）

### 文档
- `docs/parity-matrix.md` —— 新增 `@std/net/http/server` 行（partial），更新 network 汇总行（http/server 完成），更新测试计数
- `docs/session-7-progress-report.md` —— 本文件

---

## 结论

本次会话新增 1 个标准库模块与 4 个测试，把 Rust 端 `@std/*` 模块数从 60 推进到 **61**，测试总数从 165 增加到 **169**，全绿无回归。

本轮的关键贡献是**补齐了 HTTP 服务端**——这是 parity matrix 中标记为 missing 的最后一个高价值网络模块。GTS 脚本现在可以对外提供 HTTP 服务（GET/POST、JSON、自定义状态码/响应头、query/body 解析）。配合 Session 3 的 HTTP 客户端，Rust 端已具备完整的 HTTP 双向能力。

至此，所有**不依赖异步事件循环**的网络模块已全部落地。剩余高优先项（`@std/web` 框架层、完整 ES import/export、`.gspkg` 打包）属于框架级或架构级工作，不再受网络栈缺口阻塞。
