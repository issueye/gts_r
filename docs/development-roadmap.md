# GoScript Rust 重构 — 后续开发路线图

> 本文档基于 `parity-matrix.md` 与截至 2026-06-20 的实际代码状态编写，给出从当前进度到"完全功能复刻 Go 版"的接续路径、优先级排序、每阶段的验收标准与风险点。
>
> **当前状态快照**：`cargo test` 80 用例全绿，`cargo fmt --check` 通过，`cargo clippy --lib` 0 error（仍有既有 warning/建议），native registry 已接入 32 个 `@std/*` 模块；Go/Rust parity runner 已接入 51 个 fixture，其中 49 个可双跑一致，2 个保留 Rust-only 标记。

---

## 1. 当前完成度总览

### 1.1 已完成（compatible / 已验证）

| 领域 | 项 | 验证方式 |
|---|---|---|
| 构建 | `cargo build` / `cargo test` / `cargo clippy --lib` | 全绿 |
| CLI | 直接执行、`run`、`init`、REPL、`--timeout`、`--version`/`-v`、`-h` | `cli_flags.rs` (13) |
| 语言前端 | lexer / parser / AST 骨架 | 部分双跑 |
| 求值器 | 表达式、运算符、函数、闭包、类、match、错误 | `runtime_cli.rs` (9) |
| 模块 | 相对 `require`、目录模块、`project.toml` 入口、`module.exports` | `runtime_cli.rs` |
| stdlib (32) | 见 §1.2 | `stdlib_p6.rs` (23) + `stdlib_p6b.rs` (19) + `stdlib_p7.rs` (7) + `stdlib_p7b.rs` (6) |
| parity | 51 个 Rust fixture；49 个 Go/Rust 双跑一致，2 个 Rust-only | `parity_compat.rs` (2) + `scripts/parity-runner.ps1` |
| 错误处理 | try/catch 绑定 Error 不再触发 RefCell 重入 panic | `runtime_cli.rs` |

### 1.2 已接入的 32 个 `@std/*` 模块

```
@std/path @std/os @std/env @std/fs @std/json @std/time
@std/encoding/base64 @std/encoding/hex
@std/hash @std/crypto @std/random
@std/regexp @std/semver @std/collections @std/process
@std/text @std/url @std/cache @std/timers
@std/glob @std/color @std/diff @std/log @std/table @std/validation
@std/encoding/csv @std/template @std/compression @std/compress/gzip
@std/terminal @std/cli
```

### 1.3 缺口矩阵（按优先级分组）

**🔴 高优先 — 影响"能否替换 Go 版"的核心能力**

| 缺口 | 影响 |
|---|---|
| Go/Rust 双跑 fixture 继续扩容（P0） | 已达 51 个 fixture；仍需覆盖更多语法边界、错误路径和模块打包场景 |
| Rust-only fixture 差异 | 当前 `match_no_arm_catch`、`object_method_call` 不参与 Go 双跑，需后续决定对齐或记录兼容策略 |
| package resolver + `.gspkg`（P5） | 多文件项目分发不可用 |
| ES `import/export` 完整语义 | named/default/namespace/re-export 不完整 |
| 剩余 34 个 `@std/*` 模块 | 见 §2 |

**🟡 中优先 — 产品完整度**

| 缺口 | 影响 |
|---|---|
| `--workers` 调度语义 | flag 已解析，worker 池未实现 |
| `--check-types` + typechecker（P11） | 占位返回 not-implemented |
| LSP（P11） | 完全缺失 |
| CLI：`pack`/`dist`/`bundle`/`run-script`（P5） | 打包分发链缺失 |
| GTP SDK + 插件（P10） | scheduler/im-bot 生态缺失 |

**🟢 低优先 — 增强与认证**

| 缺口 | 影响 |
|---|---|
| README 对齐 Rust 版命令（P12） | 文档 |
| CI 流水线（fmt/clippy/test/parity） | 自动化 |
| 性能基线 benchmark | 无量化对比 |

---

## 2. 标准库接续计划（最大缺口）

Go 版共 66 个 `@std/*` 模块，已完成 32 个，**剩余 34 个**。按依赖与风险分批，每批目标是"可编译、可测试、可发布"。

### 批次 A — P7 格式与工具（无网络，CI 友好，**推荐下一轮**）

**目标**：复刻确定性强、无外部服务的模块。

| 模块 | 关键能力 | 难度 | 备注 |
|---|---|---|---|
| `@std/toml` | parse/stringify | 中 | 可用 `toml` crate 或自写 |
| `@std/yaml` | parse/stringify | 中 | 推荐 `serde_yaml` |
| `@std/xml` | parse/stringify | 中 | |
| `@std/markdown` | render to HTML | 中 | `pulldown-cmark` |
| `@std/schema` | schema 定义 | 中 | |
| `@std/test` | 断言/runner | 中 | 与 `@std/test` 自举 |
| `@std/archive/zip` | zip 读写 | 中 | `zip` crate |

**验收**：`examples/17-native-stdlib-cookbook.gs` 无网络部分通过；每模块至少 2 个 parity 测试。

### 批次 B — P8 系统、进程、数据库、网络（有外部依赖）

| 模块 | 关键能力 | 风险点 |
|---|---|---|
| `@std/exec` | 子进程执行 | 信号/超时语义 |
| `@std/db` | 数据库（sqlite） | 与 Go 版 sqlite 行为对齐 |
| `@std/net/http/client` | HTTP 客户端 | 推荐 `ureq`（同步、轻依赖） |
| `@std/net/http/server` | HTTP 服务端 | 推荐 `tiny_http` 或 `axum` |
| `@std/net/socket/{client,server}` | TCP/UDP | |
| `@std/net/ws/{client,server}` | WebSocket | 推荐 `tungstenite` |
| `@std/web` | 每请求独立 VM 框架 | **必须复刻每请求 VM 模型** |
| `@std/sse` | Server-Sent Events | |
| `@std/mail` | SMTP | |
| `@std/pty` | 伪终端 | 平台差异大 |
| `@std/signal` | 信号处理 | |
| `@std/watch` | 文件监听 | `notify` crate |
| `@std/runtime` | 运行时自省 | |
| `@std/prometheus` | 指标暴露 | |
| `@std/rate-limit` | 限流 | |
| `@std/retry` | 重试策略 | |
| `@std/async` | async 工具 | 依赖现有 Promise |
| `@std/stream` | 流抽象 | |
| `@std/buffer` | Buffer 类型 | **当前 base64/hex 已用 Hash 模拟，需正式化** |
| `@std/jwt` | JWT 签发/验证 | 依赖已完成的 hmac/sha |
| `@std/mime` | MIME 类型 | |
| `@std/net/ip` | IP 解析 | |
| `@std/events` | 事件 emitter | |
| `@std/express` | express 风格路由 | 依赖 `@std/net/http/server` |

**验收**：网络测试用本地端口，避免外网；Web 并发测试在 Rust 侧稳定通过。

### 批次 C — P9 TUI/图像/PDF（人工验收为主）

| 模块 | 难度 | 备注 |
|---|---|---|
| `@std/tui` | 高 | 需选 Rust TUI 栈（`crossterm`+`ratatui`） |
| `@std/image` | 中 | `image` crate |
| `@std/pdf` | 中 | |
| `@std/highlight` | 中 | 语法高亮 |

**验收**：可自动测试的部分加测试，其余建人工验收清单。

---

## 3. 阶段任务（按建议执行顺序）

### 阶段 1（已完成基础版）：修复已知 bug + 建立 parity runner

**这是最高优先，因为它是后续所有工作的地基。基础版已完成，下一步是扩容 fixture 与处理 Go/Rust 语义差异。**

1. **已完成：修复 `eval_core.rs:396` try/catch RefCell 重入借用**
   - 现象：throw 后 catch 绑定时 `e.borrow_mut()` 在已借用状态下二次借用 → panic。
   - 修复：catch 绑定时先克隆 ErrorData，再根据 `thrown` 或非 runtime Error 构造 catch 变量。
   - 验收：`session_binds_caught_error_without_refcell_reentry` 通过。

2. **已完成基础版：建立 Go/Rust 双跑 parity runner（P0）**
   - 新增 `scripts/parity-runner.ps1`：同一 fixture 先跑 Go 版 CLI，再跑 Rust 版 `gs`，比对 stdout/stderr/exit code。
   - 支持 `GTS_GO_GS` / `GTS_GO_ROOT` / 参数指定；缺少可用 Go CLI 时在测试中清晰跳过。
   - 复用 `tests/fixtures/parity/`，并接入 `optional_go_cli_matches_rust_cli`。
   - 当前验收：51 个 fixture；显式 `GTS_GO_ROOT` 下 49 个 Go/Rust 双跑一致，2 个 Rust-only。

### 阶段 2：批次 A 标准库（P7，无网络）

按 §2 批次 A 顺序推进。建议每轮迁移 5-8 个模块 + 对应 parity 测试。

**首轮已完成**：`@std/glob`（复用现有 glob_paths）、`@std/color`、`@std/diff`、`@std/log`、`@std/table`、`@std/validation`。

**第二轮已完成**：`@std/encoding/csv`、`@std/template`、`@std/terminal`、`@std/cli`、`@std/compression`、`@std/compress/gzip`。

**推荐下一轮**：`@std/toml`、`@std/yaml`、`@std/xml`、`@std/markdown`、`@std/schema`、`@std/test`、`@std/archive/zip`。

### 阶段 3：模块系统与打包（P5）

1. 迁移 `internal/module`：ModuleCache、source/native/package resolver、circular import、模块目录规则。
2. 完整 ES `import/export`：named/default/namespace/alias/re-export。
3. 迁移 `internal/proj`：project.toml 完整解析。
4. 迁移 `.gspkg`：pack/open/nested package/可执行嵌入。

**验收**：`examples/13-package-modules`、`examples/14-nested-gspkg` Rust 侧通过；`gs pack`/`gs bundle`/`gs dist` 可用。

### 阶段 4：异步与 worker（P4 加固）

1. 对齐 Promise 状态机、then/catch/finally、Promise.all/race。
2. 对齐 async/await 调度。
3. 实现 `--workers` 真实 worker 池（当前 setTimeout/setInterval 是内联阻塞，需改为调度模型）。
4. **注意**：当前 `@std/timers` 的 clear* 是 no-op，因为 VM 同步执行——阶段 4 引入真正的事件循环后需重做 timers 语义。

**验收**：`examples/11-async.gs` 通过；`--workers N` 限制并发；`--timeout` 对异步任务生效。

### 阶段 5：批次 B 标准库（P8，网络/系统）

依赖阶段 4 的事件循环就绪。按 §2 批次 B 推进，`@std/web` 的每请求 VM 模型是难点。

### 阶段 6：类型检查与 LSP（P11）

1. 实现 Rust typechecker：声明收集、类型注解检查、函数签名、class shape、module 类型。
2. 接入 `--check-types`。
3. 复刻 LSP：诊断、补全、hover（按 Go 版实际能力）。

### 阶段 7：GTP 与插件（P10）

1. 迁移 `sdk/gtp`：frame、JSON Lines、value helpers。
2. 迁移 scheduler 插件服务、im-bot 插件服务。
3. 支持项目启动配置、插件自动唤醒、脚本事件监听。

### 阶段 8：批次 C + 文档 + 发布认证（P9 + P12）

1. TUI/图像/PDF（人工验收清单）。
2. README 对齐 Rust 命令。
3. examples 全量分组（stable/manual/external-service/deprecated）。
4. CI 流水线：`cargo fmt --check`、`cargo clippy`、`cargo test`、parity fixtures。
5. 性能基线 benchmark。

---

## 4. 架构与工程约定（迁移时遵循）

### 4.1 stdlib 模块实现范式

参考 `src/stdlib/mod.rs` 既有模式：

```rust
fn xxx_module() -> Object {
    module(vec![
        ("fnName", native("xxx.fnName", xxx_fn_name)),
        // ...
    ])
}

fn xxx_fn_name(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "xxx.fnName", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    // ... 返回 str_obj / num_obj / bool_obj / array(...) / Object::Hash(...)
}
```

**关键约定**：
- 在 `load_native_module` 的 match 里注册新模块。
- 错误用 `new_error(ctx.pos.clone(), "模块.函数: 消息")`，消息前缀与 Go 版完全一致（兼容性）。
- 参数校验用 `required_string` / `required_number` / `bytes_from_object` 复用 helper。
- 字节输入（base64/hex/crypto）支持 String/Array<Number>/Buffer 三态。
- 随机源用 `fill_random`（OS RNG，Win=BCryptGenRandom，Unix=/dev/urandom）。

### 4.2 测试范式

参考 `tests/stdlib_p6.rs` / `tests/stdlib_p6b.rs` / `tests/stdlib_p7.rs`：写 `.gs` 到临时目录 → 跑 `gs` 二进制 → 断言 stdout/stderr/exit。try/catch 基础 bug 已修，可用于错误处理回归；跨 Go/Rust 语义仍需通过 parity runner 确认。

### 4.3 依赖管理

- 当前唯一外部依赖是 `regex`。
- **SHA/hash/编码已自包含实现**，未引入 crypto crate。
- P7/P8 涉及的格式/网络模块**允许引入成熟 crate**（如 `serde_yaml`/`flate2`/`ureq`），但每个新依赖需在 PR 说明理由，避免依赖膨胀。

### 4.4 Buffer 类型的技术债

当前 base64/hex/crypto 的 Buffer 用 `Hash{__buffer_data__: Array<Number>}` 模拟。Go 版有正式 `@std/buffer` 模块。**P8 应正式化 Buffer**，否则 bytes 往返有 UTF-8 lossy 风险。

---

## 5. 风险登记

| 风险 | 等级 | 缓解 |
|---|---|---|
| 同步 VM 与 Go 异步模型语义差异 | 高 | 阶段 4 需明确文档化差异；timers/clear* 在事件循环就绪后重做 |
| Go/Rust try/catch 输出差异 | 中高 | 用 parity runner 固化差异，下一轮决定对齐 Go 版或记录兼容策略 |
| 网络/DB 测试 flaky | 中 | 本地端口 + 超时；CI 隔离 |
| `@std/web` 每请求 VM 复刻 | 中高 | 单独 spike，先验证 VM clone 成本 |
| 依赖膨胀 | 中 | 每依赖评审；优先自包含 |
| TUI/PDF 跨平台 | 中 | 人工验收清单 |

---

## 6. 立即可执行的下一步（建议）

按优先级，**下一轮应做**：

1. **处理 Rust-only parity 差异**：`match_no_arm_catch` 与 `object_method_call` 暂不参与 Go 双跑，需决定对齐 Go 版或保留 Rust 行为。
2. **批次 A 剩余模块**：`@std/toml` `@std/yaml` `@std/xml` `@std/markdown` `@std/schema` `@std/test` `@std/archive/zip`。
3. **阶段 3 模块系统与打包**：开始 package resolver、完整 import/export、`.gspkg` 与 `gs pack/bundle/dist`。

完成上述三步后，项目将具备：更清晰的兼容策略、接近完整的无网络 stdlib 批次，以及进入项目分发能力的基础。

---

*本文档随进度更新。每完成一个阶段，更新对应小节的状态与验收证据。*
