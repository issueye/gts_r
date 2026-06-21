# GTS Rust 重构 — Session 3 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
继续推进 gts_r（GoScript 的 Rust 端口）对 gts（Go 版）的功能复刻，选择一批**自包含、无外部服务依赖、可在 CI 中确定性验证**的标准库模块进行落地，避免触碰需要异步事件循环或复杂打包架构的大块工作。

### 新增 5 个 `@std/*` 模块

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/rate-limit` | `create({rate, capacity})` 工厂；token bucket；`tryAcquire` / `acquire`（阻塞）/ `remaining` | `tests/stdlib_p9.rs` | 2 |
| `@std/prometheus` | `create` 工厂；`inc` / `set` / `get` / `snapshot` 指标注册表 | `tests/stdlib_p9.rs` | 2 |
| `@std/highlight` | `terminal(code, {lang, width, color})`；diff / json / shell / bash / gs / js / toml 注释着色；ANSI 转义；`color:false` 直通 | `tests/stdlib_p9.rs` | 2 |
| `@std/sse` | `parse(text)` 解析事件块；`reader(streamOrText)` 游标遍历（`next` / `readAll`）；多 `data:` 行按规范用 `\n` 合并 | `tests/stdlib_p9.rs` | 3 |
| `@std/db` | `open("sqlite", dsn)` / `drivers`；连接对象 `exec` / `query` / `queryOne` / `prepare` / `begin` / `commit` / `rollback` / `ping` / `close` | `tests/stdlib_p9_db.rs` | 5 |

### 测试结果
- **新增测试**：14 个（9 p9 + 5 p9_db），全部通过
- **测试总数**：127 → **141**，全绿
- **无回归**：原有 127 个测试全部保持通过

### 依赖
- 新增 `rusqlite = { version = "0.31", features = ["bundled"] }` —— `bundled` feature 自带 sqlite 源码编译，免去宿主环境对 sqlite 库的要求
- 新增 `[dev-dependencies] tempfile = "3"`（声明，便于后续测试复用）

---

## 实现要点与设计决策

### 1. 状态在对象模型中的存放
Rust 的 `Object` 枚举没有 Go 版 `GoObject`（任意 opaque 包装）这种通用变体，只有已知的几种类型。因此 rate-limit / prometheus / cache 等需要"可变内部状态"的模块统一采用与 `@std/cache` 一致的约定：**把状态编码进一个 `HashData`，以 `__xxx_state__` 隐藏键挂到对外返回的 Hash 实例上**，方法闭包各自 `move` clone 一份状态 `Rc<RefCell<HashData>>`。这是当前对象模型下最自然、零新增 Object 变体的做法。

### 2. `@std/db` 的 sqlite-only 范围
Go 版通过 `database/sql` + 多个驱动支持 sqlite/postgres/mysql/mssql。Rust 端仅复刻 sqlite（用 `rusqlite`），其余驱动在 `db.open` 中显式返回 `unsupported driver` 错误。这是务实的范围裁剪——网络数据库需要连接池/异步等架构，留给后续阶段；sqlite 已能覆盖大量脚本场景（配置存储、本地 ETL、测试夹具）。

为规避 `rusqlite::Connection` 的所有权问题，连接句柄使用 `Rc<UnsafeCell<Connection>>`。**安全性论证**：GTS 的 VM 是同步树遍历求值器，单线程内任何时刻只有一个调用栈在执行，因此 `UnsafeCell` 的"无别名可变借用"约束天然成立（等价于 Go 版在 mutex 保护下的单连接访问）。所有访问都通过 `unsafe fn conn_ref` 收口。

### 3. `@std/db.prepare` 的轻量实现
`rusqlite::Statement` 带有 connection 的生命周期参数，无法简单地跨方法调用存进 `Object`。当前实现把 `prepare` 做成一个**行为等价的外观对象**：每次 `exec/query/queryOne` 都重新 `prepare` 一次再执行。对同步 VM 的典型用法（少量、非热路径）没有可观察差异；如果未来出现性能问题，可改为 `Rc<UnsafeCell<Statement>>`。

### 4. `@std/sse` 的字符串输入
Go 版的 `sse.reader` 接收一个 `readableStream`（由 `@std/stream` 提供）。Rust 端 `@std/stream` 已有 `fromString`，但把底层 `BufRead` 暴露给另一个模块需要新增跨模块状态通道。为保持本次范围聚焦，Rust 端的 `sse.reader` 直接接受**字符串**或**携带 `text`/`data` 字段的对象**，内部预解析为事件列表后用游标遍历。语义与 Go 版对齐（一次性读取全部事件），后续若需要真正的流式逐行读取，可在引入跨模块 stream handle 后升级。

### 5. `@std/highlight` 的内联 wrap
Go 版复用 `@std/text` 的 `wrapToWidth`。Rust 端 `@std/text` 没有暴露独立的 wrap 辅助函数，因此 highlight 模块内置了一个简单的等宽 wrap（`wrap_simple`，按字符计数切分）。这对终端高亮的用途足够（代码行宽度限制），CJK 宽度等高级特性可后续抽取共享 helper 时再统一。

---

## 仍待完成（按优先级，承接 development-roadmap.md）

### 🔴 高优先
- `@std/net/http/server` —— 需要 per-request VM / 线程模型，是批次 B 的难点
- `@std/web` —— express 风格框架，依赖 http/server
- 完整 ES `import/export` 语义（named/default/namespace/re-export）
- package resolver + `.gspkg`（gs pack / bundle / dist）

### 🟡 中优先
- `@std/net/socket/{client,server}`、`@std/net/ws/{client,server}`
- `@std/mail`（地址解析是纯函数，可快速落地）
- `@std/signal`、`@std/watch`、`@std/runtime`（runScript/callScript，需要 VM clone）
- `--workers` 真实 worker 池
- typechecker + LSP

### 🟢 低优先
- TUI / image / pdf（人工验收为主）
- CI 流水线、性能基线

---

## 修改的文件清单

### 源代码
- `Cargo.toml` —— 新增 `rusqlite`（bundled）依赖与 `[dev-dependencies] tempfile`
- `src/stdlib/mod.rs` —— 新增 5 个模块（rate-limit / prometheus / highlight / sse / db，约 700 行），并在 `load_native_module` 注册

### 测试
- `tests/stdlib_p9.rs` —— rate-limit / prometheus / highlight / sse（9 测试）
- `tests/stdlib_p9_db.rs` —— db sqlite（5 测试）

### 文档
- `docs/parity-matrix.md` —— 新增 5 个 `compatible` 行，更新 network/db 汇总行，更新测试计数
- `docs/session-3-progress-report.md` —— 本文件

---

## 结论

本次会话新增 5 个标准库模块与 14 个测试，把 Rust 端 `@std/*` 模块数从 47 推进到 **52**，测试总数从 127 增加到 **141**，全绿无回归。新增模块聚焦"自包含、可确定性验证"，回避了需要异步事件循环或打包架构的大块工作，为后续 http/server、web 框架、打包系统等高难度工作清理了更多周边空间。
