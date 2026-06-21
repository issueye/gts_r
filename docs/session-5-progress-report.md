# GTS Rust 重构 — Session 5 进度报告

## 会话日期: 2026-06-21

---

## 本次会话完成的工作

### 目标
承接 Session 4 的节奏，继续推进 `gts_r`（GoScript 的 Rust 端口）对 `gts`（Go 版）的功能复刻。本轮聚焦**脚本自省与执行能力**这条主线，重点落地高价值的 `@std/runtime`（让 GTS 脚本能运行其他脚本），并补齐两个对齐型占位模块。

### 新增 3 个 `@std/*` 模块

| 模块 | 关键能力 | 测试文件 | 测试数 |
|---|---|---|---|
| `@std/runtime` | 子脚本执行：`runScript`/`callScript`/`runTool` | `tests/stdlib_p9_runtime.rs` | 5 |
| `@std/image` | `info(path)` 占位（对齐 Go 版 placeholder） | `tests/stdlib_p9_runtime.rs` | 1（合并） |
| `@std/pdf` | `info(path)` 占位（对齐 Go 版 placeholder） | `tests/stdlib_p9_runtime.rs` | 1（合并） |

合计 **6 个新测试**，全部通过。

### 测试结果
- **新增测试**：6 个（runtime + image/pdf 占位），全部通过
- **测试总数**：153 → **159**，全绿
- **无回归**：原有 153 个测试全部保持通过

---

## 实现要点与设计决策

### 1. `@std/runtime` —— 通过新建 Session 实现脚本隔离
Go 版 `@std/runtime` 的核心是 `runtimeExecuteScript`：每次调用都 `object.NewVirtualMachine()`、新建 module cache、配置 resolver、执行脚本、返回 `module.GetExports(env)`。这是典型的**脚本隔离**模型——子脚本不能污染父脚本的 VM 状态。

Rust 端的等价物是 `crate::runtime::Session`，它已经封装了 VM + module cache + importer 的完整装配。但 Session 原本只暴露 `run_file` / `run_source`（返回求值结果，不返回 exports）。本次给 Session 新增了两个公开方法：

```rust
pub fn run_file_for_exports(&self, file, argv, call_main) -> RuntimeResult<Object>
pub fn root_export(&self, name: &str) -> Option<Object>
```

`run_file_for_exports` 复用了 `run_source_with_options` 的装配逻辑（install_module_bindings、eval_program、call_main_if_present、wait_async），但在结尾返回 `module_exports(&self.root)` 而不是求值结果。这让 `@std/runtime` 能直接拿到子脚本的 `module.exports`。

**三个 API 的语义对齐**：
- `runScript(path, opts)` → 跑子脚本，返回 `module.exports`
- `callScript(path, name, args, opts)` → 跑子脚本，调用其 `name` 导出（args 数组）
- `runTool(path, input, opts)` → 跑子脚本，调用其 `run` 导出（input 单参）

opts 支持 `{ cwd, argv, autoMain }`，与 Go 版一致。每次调用都 `Session::new()`，确保 VM/argv/module cache 完全隔离。

### 2. `@std/image` 与 `@std/pdf` —— 对齐型占位
查看 Go 版源码发现这两个模块**本身就是占位符**——`imageInfo` 和 `pdfInfo` 都直接返回 `"basic placeholder - full implementation requires external library"` 错误。Rust 端原样复刻，parity 标记为 `compatible`（行为与 Go 版完全一致，包括"未实现"这个事实）。这避免了 parity matrix 上出现虚假的 missing 行。

### 3. 测试范式的小坑
`format!` 会把 GTS 脚本里的 `{ }` 当成格式占位符。含 `try { ... } catch` 的脚本改用 `.replace("__CHILD_PATH__", path)` 做字符串替换，避开转义地狱。这个经验已沉淀到测试文件里。

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
- `@std/pty` —— 平台差异大
- `--workers` 真实 worker 池
- typechecker + LSP

### 🟢 低优先
- TUI（crossterm+ratatui，人工验收为主）
- CI 流水线、性能基线

---

## 修改的文件清单

### 源代码
- `src/runtime/mod.rs` —— 新增 `Session::run_file_for_exports` 和 `Session::root_export` 公开方法（约 55 行）
- `src/stdlib/mod.rs` —— 新增 3 个模块（runtime / image / pdf，约 200 行），并在 `load_native_module` 注册

### 测试
- `tests/stdlib_p9_runtime.rs` —— runtime + image/pdf 占位（6 测试）

### 文档
- `docs/parity-matrix.md` —— 新增 3 个模块行（runtime compatible、image/pdf compatible），更新 network 汇总行，更新测试计数
- `docs/session-5-progress-report.md` —— 本文件

---

## 结论

本次会话新增 3 个标准库模块与 6 个测试，把 Rust 端 `@std/*` 模块数从 55 推进到 **58**，测试总数从 153 增加到 **159**，全绿无回归。

本轮的关键贡献是**打通了 GTS 脚本的脚本执行能力**：`@std/runtime` 让一个 GTS 脚本能运行、检视、调用另一个 GTS 脚本，且每次调用都通过新建 `Session` 保证 VM 级隔离。这为构建工具链（如 `gs run-script`、测试 runner、插件加载器）提供了底层支撑。两个对齐型占位模块（image/pdf）则清理了 parity matrix 上的虚假缺口。

剩余高优先项（`@std/net/http/server`、`@std/web`、完整 ES import/export、`.gspkg`）仍主要受异步事件循环和打包架构的阻塞，待后续会话处理。
