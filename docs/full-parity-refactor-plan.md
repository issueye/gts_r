# GoScript Rust 重构全量功能复刻计划

> 目标：将 `E:\codes\gts_codes\gts` Go 版 GoScript 以 Rust 在 `E:\codes\gts_codes\gts_r` 中进行功能等价重构。这里的“等价”指用户可见行为、CLI、语言语义、模块系统、标准库、打包分发、GTP 协议、示例与测试结果保持一致；内部实现可以 Rust 化，但不得牺牲兼容性。

## 1. 当前重构态势

### 1.1 Go 原版能力面

Go 版已经是完整独立解释器产品线，主要能力包括：

- **语言前端**：lexer、parser、AST、类型注解解析、模式匹配、类、函数、闭包、异常、异步语法。
- **运行时**：树遍历求值器、VM、Environment、ObjectManager、Promise、async/await、timers、资源超时与 worker 控制。
- **模块系统**：`require(path)`、ES 风格 `import/export` 基础语义、模块缓存、package/project 解析、`.gspkg`、嵌入式可执行文件。
- **CLI**：直接执行、`run`、`run-script`、`init`、`pack`、`dist`、`bundle`、REPL、LSP、`--timeout`、`--workers`、`--check-types`、`--version`。
- **标准库**：大量 `@std/*` 原生模块，覆盖 fs/path/os/process/exec/crypto/db/http/socket/ws/web/tui/test/encoding/env/log/time/json/yaml/toml 等。
- **GTP 与插件**：`sdk/gtp` JSON Lines 协议、scheduler、im-bot 插件服务、脚本事件/自动唤醒示例。
- **验证资产**：Go 单测、CLI 回归、stdlib 测试、examples 与 docs examples。

### 1.2 Rust 版当前状态

Rust 版目前已经具备：

- **已有骨架**：lexer、parser、AST、object、evaluator、promise、builtins、runtime、CLI。
- **可编译**：`cargo check` 通过。
- **最小可运行 CLI**：`gs <file.gs>`、`gs run [file.gs]`、`--help`、`--version`。
- **最小运行时 Session**：可解析执行源码/文件，支持相对 `require()`、基础模块缓存、`process.argv`。
- **基础内置**：`console`、`println`/`print`、Math/JSON/Object/Array/String/Number/Boolean、Error、Promise、Date、timers 等部分全局。
- **新增测试**：`tests/runtime_cli.rs` 覆盖源码执行、相对模块 `require`、`project.toml entry`。

### 1.3 核心判断

Rust 版当前处于 **P1/P2 早期：语言核心原型 + 最小 CLI**。距离“完全功能复刻”主要缺口不在 lexer/parser 文件数量，而在：

1. CLI 产品能力未复刻。
2. 模块解析与包分发未复刻。
3. 标准库规模差距最大。
4. GTP/插件生态未复刻。
5. Go 版测试资产尚未系统迁移。
6. 语义兼容缺少金标准对照测试。

## 2. 功能复刻缺口矩阵

| 领域 | Go 原版 | Rust 当前 | 复刻状态 |
|---|---|---|---|
| 构建基线 | `go test ./...` | `cargo check/test` 可过 | 部分 |
| CLI 执行文件 | 支持 | 支持 | 基本完成 |
| `gs run` 项目入口 | project.toml | 简易 entry 解析 | 部分 |
| 自动调用 `main()` | 支持 | 未完整支持 | 缺口 |
| `--timeout` | 支持 | 未支持 | 缺口 |
| `--workers` | 支持 | 未支持 | 缺口 |
| `--check-types` | 接受并返回明确状态/后续接入 | 未支持 | 缺口 |
| REPL | `.help`/`.exit`/`.load`/持久环境 | 未支持 | 缺口 |
| LSP | 支持 | 未支持 | 缺口 |
| `init` | 支持 | 未支持 | 缺口 |
| `bundle` | 支持 | 未支持 | 缺口 |
| `pack`/`.gspkg` | 支持 | 未支持 | 缺口 |
| `dist`/嵌入包 | 支持 | 未支持 | 缺口 |
| lexer/parser | 基本完整 | 已迁移较多，需对照测试 | 部分 |
| evaluator 语义 | 大量覆盖 | 已有核心，未系统对照 | 部分 |
| `require` 文件模块 | 支持 | 基础支持 | 部分 |
| ES `import/export` | 基础支持 | evaluator 有基础，loader 简化 | 部分 |
| package resolver | 完整 | 未支持 | 缺口 |
| native `@std/*` | 大量支持 | 未接入 native module registry | 缺口 |
| async worker/event loop | 支持 | Promise/timer 雏形 | 部分 |
| web 每请求 VM | 支持 | 未支持 | 缺口 |
| typechecker | 计划/部分目录 | 未支持 | 缺口 |
| GTP SDK | 支持 | 未支持 | 缺口 |
| plugins | scheduler/im-bot | 未支持 | 缺口 |
| 示例回归 | Go 有稳定集 | Rust 仅 3 个测试 | 缺口 |

## 3. 完全复刻原则

1. **先建立金标准**：所有用户可见行为以 Go 版当前行为为准，而不是只参考文档目标行为。
2. **先测后迁**：每迁移一个模块，先建立 Go/Rust 双跑测试用例。
3. **避免一次性大爆炸**：标准库按依赖层与风险分组迁移。
4. **兼容优先于重构美感**：Rust 内部可重构，但 CLI 输出、错误类型、模块导出、示例行为要兼容。
5. **每阶段必须可发布**：每阶段结束时 Rust 版都应可编译、可运行、测试清单明确。

## 4. 阶段开发计划

### P0：盘点与金标准基线

**目标**：建立 Go/Rust 对照验证体系，锁定复刻边界。

**任务**

- 整理 Go 版 CLI 命令、flags、退出码、错误输出格式。
- 从 Go 版 `cmd/gs/main_test.go`、`internal/*/*_test.go`、`examples/README.md` 抽取兼容用例。
- 建立 Rust 侧 `tests/fixtures` 与双跑脚本：同一个 `.gs` 先跑 Go 版，再跑 Rust 版，比对 stdout/stderr/exit code。
- 建立功能状态表：`parity-matrix.md`，每项标记 `missing / partial / compatible / intentionally-different`。
- 确定不复刻或已废弃项目：例如 Go 版 README 中声明移除的旧 embedding API、`@go/* @host/* @plugin/*` 进程内 ABI。

**验收**

- `cargo test` 通过。
- 至少 20 个基础语言双跑用例建立。
- 每个后续阶段都有对应测试分组。

### P1：CLI 与运行时入口完全对齐

**目标**：Rust `gs` 的入口行为达到 Go 版 CLI 基线。

**任务**

- 支持全局 flags：`--version`、`--timeout`、`--workers`、`--check-types`、`--help`。
- 支持命令：直接执行、`run`、`run-script`、`init`。
- 无参数进入 REPL，而不是只打印 help。
- `gs run` 完整读取 `project.toml`，支持默认入口、参数、工作目录语义。
- 直接执行 `main.gs` 或项目入口时自动调用顶层 `main()`，对齐 Go 版行为。
- 统一错误打印与退出码。

**验收**

- Rust CLI 兼容 Go 版基础 CLI 测试。
- `examples/01-basics.gs`、docs 基础示例可通过 Rust 运行。
- `--timeout` 能中止死循环脚本。

### P2：语言前端语法复刻

**目标**：lexer/parser/AST 对 Go 版语法完全兼容。

**任务**

- 移植 Go 版 lexer/parser 单测到 Rust。
- 对照 `docs/grammar.ebnf` 与 Go parser 行为，覆盖：
  - 变量/const/var、类型注解。
  - 函数、箭头函数、默认参数、rest/spread。
  - 类、继承、字段、静态成员、`this`/`super`。
  - `if/while/for/for-in/for-of`。
  - `try/catch/finally`、`throw`。
  - `match` 模式。
  - 模板字符串、regexp。
  - ES `import/export`。
- 消除当前 parser 中“跳过 destructuring”等静默降级，若 Go 版支持则实现，不支持则错误对齐。

**验收**

- Go parser 测试的 Rust 迁移版通过率 100%。
- 所有 docs grammar 样例可解析或按 Go 版输出错误。

### P3：求值器核心语义复刻

**目标**：基础语言运行行为与 Go 版一致。

**任务**

- 移植 evaluator 测试：表达式、运算符、对象、数组、函数、闭包、作用域、类、继承、错误处理。
- 对齐严格语义：
  - 禁用 `==`/`!=`。
  - 未声明赋值报错。
  - const 赋值报错。
  - `+` 的类型规则。
  - `break/continue/return` 边界错误。
- 完整 Error 对象与 stack、子类、`throw` 非 Error 值包装。
- 对齐 console 方法与 stdout/stderr。
- 处理 C-style `for` 边角、循环作用域。

**验收**

- Go 版 `01` 到 `10` 教学示例在 Rust 侧通过。
- 基础 evaluator 双跑用例输出一致。

### P4：异步、Promise 与事件循环复刻

**目标**：`Promise`、`async/await`、timers 行为与 Go 版一致。

**任务**

- 对齐 Promise 状态机、then/catch/finally、resolve/reject、Promise.all/race 等。
- 对齐 `async function` 返回 Promise 与 await 阻塞/调度行为。
- 支持 `setTimeout`、`setInterval`、`clearTimeout`、`clearInterval`、`sleepAsync`。
- 实现 worker 数限制与 Session 生命周期 drain。
- 建立死锁、长任务、异常传播测试。

**验收**

- `examples/11-async.gs` 可运行。
- Go 版 async/timer 相关测试迁移通过。
- `--workers` 与 `--timeout` 对异步任务生效。

### P5：模块系统、resolver 与 package 复刻

**目标**：多文件项目、package、`.gspkg` 行为一致。

**任务**

- 迁移 `internal/module`：
  - `ModuleCache`
  - source/native/package resolver
  - package manifest
  - circular import 行为
  - module dir 与相对路径规则
- 完整 `require()`、`exports`、`module.exports` 兼容行为。
- 完整 ES `import/export`：
  - named/default/namespace import
  - alias
  - re-export
  - export declaration/specifier/default
- 迁移 `internal/proj` project.toml parser。
- 迁移 `internal/bundle` bundler。
- 迁移 packagefile：`.gspkg` pack/open/nested package/嵌入可执行读取。

**验收**

- `examples/13-package-modules`、`examples/14-nested-gspkg` Rust 侧通过。
- Go module resolver 测试 Rust 迁移版通过。
- `gs pack`、`gs bundle`、`gs dist` 可用。

### P6：核心标准库第一批（无网络、低外部依赖）

**目标**：先复刻确定性强、CI 友好的 `@std/*`。

**模块**

- `@std/fs`
- `@std/path`
- `@std/os`
- `@std/process`
- `@std/env`
- `@std/json`
- `@std/encoding/base64`
- `@std/encoding/hex`
- `@std/crypto`
- `@std/hash`
- `@std/random`
- `@std/time`
- `@std/timers`
- `@std/text`
- `@std/url`
- `@std/regexp`
- `@std/semver`
- `@std/cache`
- `@std/collections`

**任务**

- 建立 native module registry。
- 对每个模块按 Go 文件函数导出逐项复刻。
- 输出与错误类型必须对齐 Go 版。

**验收**

- `examples/16-native-stdlib.gs` 中对应功能通过。
- 第一批 stdlib 双跑测试通过。

### P7：扩展标准库第二批（格式、工具、终端、测试）

**模块**

- `@std/encoding/csv`
- `@std/toml`
- `@std/yaml`
- `@std/xml`
- `@std/markdown`
- `@std/template`
- `@std/schema`
- `@std/validation`
- `@std/log`
- `@std/table`
- `@std/color`
- `@std/terminal`
- `@std/cli`
- `@std/test`
- `@std/diff`
- `@std/glob`
- `@std/compression`
- `@std/compress/gzip`
- `@std/archive/zip`

**验收**

- `examples/17-native-stdlib-cookbook.gs` 中无网络部分通过。
- Go stdlib 对应测试迁移通过。

### P8：系统、进程、数据库与网络标准库

**模块**

- `@std/exec`
- `@std/db`
- `@std/net/http/client`
- `@std/net/http/server`
- `@std/net/socket/client`
- `@std/net/socket/server`
- `@std/net/ws/client`
- `@std/net/ws/server`
- `@std/web`
- `@std/sse`
- `@std/mail`
- `@std/pty`
- `@std/signal`
- `@std/watch`
- `@std/runtime`
- `@std/prometheus`
- `@std/rate-limit`
- `@std/retry`
- `@std/async`
- `@std/stream`
- `@std/buffer`
- `@std/jwt`
- `@std/mime`
- `@std/net/ip`

**任务**

- 明确 Rust crate 选择与平台差异。
- 网络/server 测试使用本地端口，避免外网。
- `@std/web` 必须保持每请求独立 VM 模型。
- 对 DB 选择与 Go 版 sqlite 行为对齐。

**验收**

- Go 网络/stdlib 相关测试可迁移通过。
- Web 并发测试可在 Rust 侧稳定通过。

### P9：TUI、图像/PDF/高阶能力复刻

**模块**

- `@std/tui`
- `@std/image`
- `@std/pdf`
- `@std/highlight`
- 其他 UI/终端深度能力。

**任务**

- 确定 Rust TUI 技术栈与 Go 版行为映射。
- 分离可自动测试与人工验证。
- 保持 API 兼容，必要时内部降级但文档标注。

**验收**

- Go 版 TUI/terminal/highlight 测试可迁移通过或有明确人工验收清单。

### P10：GTP SDK 与插件生态复刻

**目标**：Rust 版支持 Go 版 GTP 协议与插件交互模型。

**任务**

- 迁移 `sdk/gtp`：
  - frame
  - JSON Lines
  - value helpers
  - request/response/event 语义
- 迁移/复刻 `internal/gtp` re-export。
- 迁移 scheduler 插件服务。
- 迁移 im-bot 插件服务。
- 支持项目启动配置、插件自动唤醒、脚本事件监听。

**验收**

- Go GTP frame 测试 Rust 版通过。
- scheduler 示例、脚本事件示例 Rust 版通过。

### P11：类型检查、静态分析与 LSP

**目标**：复刻 CLI 中的 `--check-types` 与 LSP 能力。

**任务**

- 根据 Go 文档/现有 typechecker 目录行为定义类型系统边界。
- 实现 Rust typechecker：
  - 声明/作用域收集
  - 类型注解检查
  - 函数参数/返回
  - class/interface/object shape 基础检查
  - module export/import 类型检查
- 接入 `--check-types`。
- 复刻 LSP：诊断、语法高亮辅助、补全/hover（按 Go 版实际能力）。

**验收**

- `gs --check-types` 行为对齐 Go 版目标/当前状态。
- LSP smoke test 可启动并响应基础请求。

### P12：文档、示例、发布与最终兼容认证

**目标**：完成可替换 Go 版的 Rust 版交付。

**任务**

- README 对齐 Rust 版命令。
- examples 全量分组：
  - stable
  - manual
  - external-service
  - intentionally-deprecated
- 全量迁移 Go 测试或建立等价 Rust 测试。
- 建立 CI：
  - `cargo fmt --check`
  - `cargo clippy`
  - `cargo test`
  - parity fixtures
  - selected integration tests
- 性能基线：
  - lexer/parser benchmark
  - module cache benchmark
  - stdlib benchmark
  - web 并发 benchmark

**验收**

- 主要 Go examples 在 Rust 侧通过。
- Rust 版 CLI 可执行常见项目。
- parity matrix 达到 100% compatible 或明确 documented exception。

## 5. 推荐优先级路线

短期不要先攻所有 `@std/*`。推荐顺序：

1. **CLI 完整化**：用户入口先对齐，后面所有验证才有共同跑道。
2. **语言语义对照测试**：避免 stdlib 迁移时被底层语义差异拖垮。
3. **模块/package**：这是 stdlib 与真实项目运行的前置条件。
4. **无网络 stdlib**：先吃确定性模块，快速扩大 examples 通过率。
5. **异步/network/web**：复杂但价值高，放到基础稳定后。
6. **GTP/LSP/TUI**：生态和工具链阶段。

## 6. 测试策略

### 6.1 双跑 parity fixture

每个 fixture 包含：

- `input.gs`
- 可选 `project.toml`
- 可选模块文件
- `expected.stdout`
- `expected.stderr`
- `expected.exit`
- `features.toml`

运行器负责：

1. 调用 Go 版 `gs` 得到金标准。
2. 调用 Rust 版 `gs`。
3. 比对输出、退出码、错误类型。

### 6.2 分层测试

- **unit**：lexer/parser/object/evaluator 单元。
- **runtime**：Session、module cache、async drain。
- **cli**：命令、flags、cwd、exit code。
- **stdlib**：每个 `@std/*` 单独测试。
- **examples**：教学示例与 docs 示例。
- **integration**：package、web、GTP、plugins。

### 6.3 阶段质量门

每阶段完成前必须：

- `cargo fmt --check` 通过。
- `cargo test` 通过。
- 当前阶段 parity fixtures 通过。
- `docs/parity-matrix.md` 更新。

## 7. 规模与风险

### 7.1 最大风险

- **标准库规模巨大**：Go 版 `internal/stdlib` 模块很多，且覆盖网络、终端、DB、web、TUI。
- **异步模型差异**：Go goroutine/channel 与 Rust async/thread 模型不能机械翻译。
- **包与嵌入式可执行**：`.gspkg`、dist、nested package 需要先设计 Rust 文件格式兼容策略。
- **错误输出兼容**：用户脚本依赖错误字符串时，微小差异也会造成不兼容。
- **平台差异**：Windows shell、PTY、terminal、signal、path 行为需特别测试。

### 7.2 控制措施

- 每个高风险模块先写兼容测试，再迁移。
- native stdlib 分批迁移，不阻塞语言核心。
- 对网络/终端/DB 使用 feature gate 或 test category。
- 保留 parity matrix，防止“看起来实现了但行为不一致”。

## 8. 当前下一步建议

建议立即进入 P0/P1：

1. 建立 `docs/parity-matrix.md`。
2. 建立 parity fixture runner。
3. 完成 CLI flags、REPL 入口、`main()` 自动调用。
4. 将 Go 版 `examples/01-basics.gs` 到 `06-arrays-objects.gs` 纳入 Rust 双跑。
5. 再推进 module resolver，而不是马上扩 stdlib。
