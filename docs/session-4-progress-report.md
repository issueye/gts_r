# GTS Rust 重构 — Session 4 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
承接 Session 3 的节奏，继续推进 `gts_r`（GoScript 的 Rust 端口）对 `gts`（Go 版）的功能复刻。本轮聚焦**网络与协议解析**这条主线，挑选了既高价值又能在同步 VM 下确定性验证的模块。

### 新增 3 个 `@std/*` 模块

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/mail` | RFC 5322 邮件地址/消息解析与格式化 | `tests/stdlib_p9_mail.rs` | 8 |
| `@std/net/socket/client` | 同步 TCP 客户端（阻塞 I/O、DNS、超时） | `tests/stdlib_p9_socket.rs` | 1（共享） |
| `@std/net/socket/server` | 同步 TCP 服务端（listen + acceptOne） | `tests/stdlib_p9_socket.rs` | 3（共享） |

合计 **12 个新测试**，全部通过。

### 测试结果
- **新增测试**：12 个（8 mail + 4 socket），全部通过
- **测试总数**：141 → **153**，全绿
- **无回归**：原有 141 个测试全部保持通过

---

## 实现要点与设计决策

### 1. `@std/mail` —— 纯函数，零依赖
完全自包含，未引入任何外部 crate。地址解析手写了一个最小 RFC 5322 解析器：
- 支持 `Name <addr@domain>` 和裸 `addr@domain` 两种形式
- `parseAddressList` 按"顶层逗号"切分（跳过引号和尖括号内的逗号）
- `formatAddress` 在显示名含逗号或引号时自动加引号转义
- `parseMessage` 按"空行"切分头/体，并支持 RFC 5322 头折叠（续行以空格/制表符开头）
- `formatDate` 用 `format_rfc1123z` 直接输出 RFC 1123Z（`Mon, 02 Jan 2006 15:04:05 -0700`）——因为现有 `format_time_layout` 不支持该布局（只支持几种固定 Go 布局），内联实现了 weekday/month 查表（1970-01-01 = Thursday 作为锚点）

### 2. `@std/net/socket/client` —— 同步 VM 下的阻塞 I/O
Go 版用 `net.DialTimeout`。Rust 端：
- `connect(host, port)` 先 `resolve_socket_addr`（支持 IP 字面量和主机名 DNS 解析），再 `connect_timeout`（30s）
- 连接句柄存进 `SocketStream { stream: RefCell<Option<TcpStream>> }`，方法闭包各自 clone 一份 `Rc`
- `read`/`recv`/`write`/`send`/`close`/`setDeadline` 与 Go 版一一对应
- `read` 在 EOF 时返回 `null`（对齐 Go 版 `io.EOF → NULL`）
- `setDeadline` 用 `set_read_timeout`/`set_write_timeout` 模拟（同步模型下"绝对时刻"等价于相对超时）

### 3. `@std/net/socket/server` —— 同步 VM 下的事件循环妥协
**这是本次最难的设计决策。** Go 版用 goroutine 跑后台 accept 循环，但 Rust 的 GTS VM 是**纯同步树遍历器，没有事件循环、没有 spawn 能力**。因此无法复刻 Go 版的"listen 后自动接收多连接"语义。

务实方案：
- `listen(port, handler?)` 绑定监听器，**设为非阻塞**，把 handler（可选）存进 server 对象后立即返回（不阻塞、不 spawn）
- 新增 `acceptOne(handler?)` 方法：**同步阻塞接收单个连接**，调用 handler 处理后返回。handler 可在 listen 注册，也可在 acceptOne 显式传入
- 无 pending 连接时返回 `WouldBlock` 错误（非阻塞 listener 的语义），让调用方能区分"没连接"和"出错"

**与 Go 版的差异已在 parity-matrix 标注为 `partial`**：Go 版是并发多连接服务器，Rust 版是"单次同步 accept"模型。完整并发需要引入事件循环（roadmap 阶段 4），届时 server 语义需要重做。但对脚本侧"启服务、接一个连接、echo、关"这类确定性场景已完全可用，echo 回环测试验证了这一点。

### 4. 连接对象的统一构造
`new_socket_conn_object` 同时服务 client 和 server 端的连接，确保两侧 API 完全一致（`write`/`read`/`close`/`setDeadline` 等），减少脚本侧的心智负担。

---

## 仍待完成（按优先级）

### 🔴 高优先
- `@std/net/http/server` —— 需要 per-request VM / 线程模型（与 socket server 同样的异步障碍）
- `@std/web` —— express 风格框架，依赖 http/server
- 完整 ES `import/export` 语义（named/default/namespace/re-export）
- package resolver + `.gspkg`（gs pack / bundle / dist）

### 🟡 中优先
- `@std/net/ws/{client,server}` —— WebSocket（推荐 tungstenite）
- `@std/signal`、`@std/watch` —— 需要事件循环，当前不可同步复刻
- `@std/runtime`（runScript/callScript）—— 需要 VM clone
- `--workers` 真实 worker 池
- typechecker + LSP

### 🟢 低优先
- TUI / image / pdf（人工验收为主）
- CI 流水线、性能基线

---

## 修改的文件清单

### 源代码
- `src/stdlib/mod.rs` —— 新增 3 个模块（mail / net/socket/client / net/socket/server，约 600 行），并在 `load_native_module` 注册；内联实现 `format_rfc1123z`、`weekday_short`、`month_short`、`resolve_socket_addr` 等辅助函数

### 测试
- `tests/stdlib_p9_mail.rs` —— mail 模块（8 测试）
- `tests/stdlib_p9_socket.rs` —— socket client/server（4 测试，含 echo 回环）

### 文档
- `docs/parity-matrix.md` —— 新增 3 个模块行（mail compatible、socket client/server partial），更新 network 汇总行，更新测试计数
- `docs/session-4-progress-report.md` —— 本文件

---

## 结论

本次会话新增 3 个标准库模块与 12 个测试，把 Rust 端 `@std/*` 模块数从 52 推进到 **55**，测试总数从 141 增加到 **153**，全绿无回归。

本轮的关键贡献是**打通了 Rust 端的网络通信能力**：HTTP 客户端（Session 3）+ TCP 客户端/服务端（本轮）让 GTS 脚本能进行进程间通信；mail 模块补齐了协议解析侧的拼图。socket server 的"同步 acceptOne"模型诚实记录了与 Go 版并发语义的差异，为后续引入事件循环后的重做留出了清晰接口。
