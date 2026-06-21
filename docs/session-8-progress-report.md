# GTS Rust 重构 — Session 8 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
承接 Session 7（HTTP 服务端已落地），本轮在 http/server 之上叠加 **Express 风格的 Web 框架**，让 GTS 脚本能用熟悉的 `app.get/post/use/listen` 路由模型开发 HTTP 服务。

### 新增 1 个 `@std/*` 模块（含别名）

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/web`（`@std/express` 别名） | Express 风格框架：`createApp` + 路由 + 中间件 + `listen` | `tests/stdlib_p9_web.rs` | 5 |

合计 **5 个新测试**，全部通过。

### 测试结果
- **新增测试**：5 个（web），全部通过
- **测试总数**：169 → **174**，全绿
- **无回归**：原有 169 个测试全部保持通过

---

## 实现要点与设计决策

### 1. 同步 Express 模型
Go 版 `@std/web`（1149 行）依赖**每请求独立 VM + 并发信号量**的复杂隔离模型——这在 Rust 的同步树遍历 VM 里无法复刻。本次实现了一个**同步等价物**：
- `createApp()` 返回一个 app 对象，内部维护有序路由表（`RefCell<Vec<WebRoute>>`）
- `app.get/post/put/patch/delete/all(path, handler)` 注册路由（path 按 `/` 分段，支持 `:param` 捕获）
- `app.use([path], handler)` 注册中间件（按前缀匹配）
- `app.listen(port, {count: N})` 绑定 tiny_http 服务器，**同步处理 N 个请求后返回**（默认 1），让脚本可以继续执行或退出

这给出了一个可在单线程同步 VM 里跑起来的 Express 体验，而非 Go 版的并发服务器。parity 标记 `partial` 如实反映这个差异。

### 2. 路由匹配
- `exact_match`：路由段数必须等于请求段数，`:name` 段捕获对应请求段为参数，其余段精确比较
- `prefix_match`（中间件）：请求路径必须以挂载路径为前缀
- 匹配的路由按注册顺序进 chain，handler 依次执行；某 handler 写了响应体后 chain 停止
- 无任何路由匹配 → 自动 404；handler 抛运行时错误 → 自动 500

### 3. Context 对象
每个 handler 收到 `{req, res, params}`：
- `req`：`{method, url, path, body, query, headers}`
- `res`：`{status, setHeader, send, json, end}`（复用 http/server 的 `http_response_object`，闭包捕获共享 `HttpResponseState`）
- `params`：路径参数 Hash（`{id: "42"}`）

### 4. 辅助函数
- `web.json(obj)` → JSON 字符串（复用内联序列化器）
- `web.text(str)` → 原样返回（文档意图）
- `web.static(root)` → 返回一个 handler 函数（路径穿越防护：canonicalize 后检查 `starts_with(root)`；当前因响应状态架构限制，静态服务建议脚本用 `@std/fs` 读文件后 `res.send()`）

### 5. tiny_http Request 所有权处理
`tiny_http::Request::respond(self)` 按值消费 request，而读取 body/headers 需要 `&self`。解决方案：`web_handle_request` 按**值**接收 request，先借用读取所有元数据，最后 `request.respond(response)` 一次性消费。避免了 unsafe 的 `ptr::read` 把戏。

### 6. 测试策略：固定端口 + 子进程
与 http_server 测试一致：GTS 服务端脚本作为子进程启动，`println("GTS_PORT=port")` 在 `listen` 之前打印固定端口，测试线程 sleep 100ms 后用裸 TcpStream 发 HTTP 请求。5 类测试覆盖：GET+text、路径参数+JSON、404、中间件链、`web.json` 序列化。

---

## 仍待完成（按优先级）

### 🔴 高优先
- 完整 ES `import/export` 语义（named/default/namespace/re-export）
- package resolver + `.gspkg`（gs pack / bundle / dist）
- `@std/signal`、`@std/watch`（需要事件循环）

### 🟡 中优先
- `@std/pty` —— 平台差异大
- `--workers` 真实 worker 池
- typechecker + LSP
- language parity：errors/stack 子类对齐、classes 更多测试对照

### 🟢 低优先
- TUI（crossterm+ratatui，人工验收为主）
- CI 流水线、性能基线

---

## 修改的文件清单

### 源代码
- `src/stdlib/mod.rs` —— 新增 `@std/web` + `@std/express` 模块（约 470 行：createApp、路由注册、listen、中间件、路径匹配、context 对象、json/text/static 辅助），并在 `load_native_module` 注册两个别名

### 测试
- `tests/stdlib_p9_web.rs` —— web 框架（5 测试，含子进程服务端+测试线程客户端的真实 HTTP 回环）

### 文档
- `docs/parity-matrix.md` —— 新增 `@std/web` 行（partial），更新 network 汇总行（web 完成），更新测试计数
- `docs/session-8-progress-report.md` —— 本文件

---

## 结论

本次会话新增 1 个标准库模块与 5 个测试，把 Rust 端 `@std/*` 模块数从 61 推进到 **62**（`@std/express` 是别名，共享实现），测试总数从 169 增加到 **174**，全绿无回归。

本轮的关键贡献是**补齐了 Express 风格的 Web 框架**。GTS 脚本现在可以用熟悉的 `app.get("/users/:id", handler)` 模型开发 HTTP 服务，支持路径参数、中间件链、JSON 响应、404/500 自动处理。配合 Session 7 的 http/server 底层，Rust 端已具备完整的 Web 开发能力。

至此，**所有可在同步 VM 下落地的网络/Web 模块已全部完成**（http client/server、socket client/server、ws client/server、web 框架、runtime、exec、db、mail、sse）。剩余高优先项（完整 ES import/export、`.gspkg` 打包、typechecker/LSP）属于语言核心或工具链架构工作，不再受网络/Web 栈缺口阻塞。
