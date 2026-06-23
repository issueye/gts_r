# GoScript Rust Parity Matrix

> 状态枚举:`missing` 未实现;`partial` 部分实现;`compatible` 已按 Go 版行为验证;`blocked` 需要设计/依赖;`deprecated` Go 版已废弃或明确不复刻。
>
> 本文以 2026-06-23 代码状态为准。历史版本曾标注大量 missing/partial(Date/Map/Set/网络/字节码),实际**已实现**,此处已校正。

| Area | Item | Status | Notes |
|---|---|---:|---|
| Build | `cargo build` / `cargo test` / `cargo clippy` | compatible | 全绿;默认启用 tokio feature |
| CLI | direct file execution | compatible | `gs <file.gs>`,支持字节码(默认)/树遍历 |
| CLI | `--exec-mode` | compatible | `bytecode`(默认)/ `tree`(legacy fallback),`cli_flags.rs` 验证 |
| CLI | `run` project entry | partial | 简易 `project.toml entry` 支持,需迁移完整 Go `internal/proj` |
| CLI | `run-script` | compatible | `gs run-script <script.gs> [args...]` 自动调用 main 函数 |
| CLI | `init` | compatible | 脚手架与 project.toml,`cli_flags.rs` 验证 |
| CLI | `pack` | partial | 部分实现,依赖 `.gspkg` |
| CLI | `dist` | partial | 依赖 packagefile append |
| CLI | `bundle` | partial | bundler.rs 已有,链路待完善 |
| CLI | REPL | compatible | `.help/.exit` 持久会话,`cli_flags.rs` 验证 |
| CLI | LSP | missing | Go 版 `internal/lsp` |
| CLI | `--timeout` | compatible | 能中止死循环,`cli_timeout_stops_infinite_loop` 验证 |
| CLI | `--workers` | partial | flag 已透传 + `@std/web` prefork 已用;其他场景调度语义待统一 |
| CLI | `--check-types` | missing | 占位返回 not-implemented |
| CLI | `-v` / `--version` | compatible | 输出 `bytecode + tokio-io`,`cli_flags.rs` 验证 |
| Language | lexer | compatible | 已迁移 |
| Language | parser | compatible | 完整语法覆盖 |
| Language | evaluator (tree, legacy fallback) | compatible | 作为 `--exec-mode=tree` 路径保留 |
| Language | bytecode VM (默认后端) | compatible | 阶段 0–11 完成,全量 AST 覆盖,VM 单跑全绿 |
| Language | classes/inheritance/super | compatible | 构造器/继承/字段/静态成员/super,VM 已下沉 |
| Language | closures/upvalue | compatible | 开放/闭合两态 upvalue,native↔VM 互调 |
| Language | match | compatible | literal/ident/wildcard/or/range + guard |
| Language | try/catch/finally + throw | compatible | VM 已下沉 |
| Language | template/regexp | compatible | |
| Language | nullish coalescing `??` | compatible | 已补入 VM(阶段 11.1) |
| Language | errors/stack | partial | 需对齐 Error 子类与 stack |
| Async | Promise | compatible | then/catch/finally/all/race/allSettled,`async_promise.rs` 验证 |
| Async | async/await | compatible | bytecode_async.rs 验证 |
| Async | timers | compatible | setTimeout/setInterval/sleepAsync;tokio feature 下事件循环驱动 |
| Async | completion queue 桥接 | compatible | VM 线程 drain completion,Tokio 投递 Send 结果,`async_completion.rs` 验证 |
| Modules | relative `require` | compatible | 缓存/circular/module.exports 已对齐 |
| Modules | ES import/export | partial | named/default/namespace 基础,re-export/alias 待补 |
| Modules | native `@std/*` registry | compatible | 已接入 68 个模块(见下方 Stdlib 明细) |
| Modules | package resolver | partial | 基础支持,完整迁移 Go `internal/module/resolver` 待补 |
| Packaging | `.gspkg` | partial | pack/open 部分实现,nested/embed 待完善 |
| Packaging | nested package | partial | |
| Packaging | executable embedding | partial | dist 部分实现 |
| Stdlib | core globals | compatible | Math/JSON/Object/Array/String/Number/Boolean/Map/Set/Date/Error/Symbol |
| Stdlib | 68 个 `@std/*` 模块 | compatible | 逐模块见下表 |
| Stdlib | `@std/fs` | compatible | enhanced API(walkSync/globSync/createThrottledWriter…) |
| Stdlib | `@std/path` | compatible | |
| Stdlib | `@std/os` | compatible | |
| Stdlib | `@std/env` | compatible | 16 函数(get/getInt/.../require 支持数组) |
| Stdlib | `@std/json` | compatible | parse5/validate/get/set/patch/diff |
| Stdlib | `@std/time` | compatible | parse/format/add/parseDuration/unix/sleep |
| Stdlib | `@std/timers` | compatible | tokio 下事件循环驱动 |
| Stdlib | `@std/encoding/{base64,hex,csv}` | compatible | |
| Stdlib | `@std/hash` | compatible | adler32/crc32(IEEE)/crc64(ISO)/fnv1a 已知向量核对 |
| Stdlib | `@std/crypto` | compatible | sha1/256/512/hmac/pbkdf2(NIST/RFC 向量核对)/randomUUID/randomBytes |
| Stdlib | `@std/random` | compatible | int/float/bool/pick/sample/shuffle/hex/base64/uuid/bytes |
| Stdlib | `@std/regexp` | compatible | escape/matchAll/split |
| Stdlib | `@std/semver` | compatible | parse/compare/gt/lt/eq/inc/satisfies |
| Stdlib | `@std/collections` | compatible | unique/chunk/flatten/sample/shuffle/range |
| Stdlib | `@std/process` | compatible | argv/pid/version/cwd/getenv/hrtime/… |
| Stdlib | `@std/cache` | compatible | create 工厂 + set/get/has/delete/clear/size/keys,毫秒 TTL |
| Stdlib | `@std/text` | compatible | chars/runes/width/truncateWidth/wrapWidth/stripAnsi(CJK=2) |
| Stdlib | `@std/url` | compatible | parse/format/resolve/pathToFileURL/fileURLToPath |
| Stdlib | `@std/glob` | compatible | glob 通配匹配 |
| Stdlib | `@std/color` | compatible | ANSI 颜色输出 |
| Stdlib | `@std/diff` | compatible | 文本/对象 diff |
| Stdlib | `@std/log` | compatible | 结构化日志 |
| Stdlib | `@std/table` | compatible | ASCII 表格渲染 |
| Stdlib | `@std/validation` | compatible | 校验器 |
| Stdlib | `@std/template` | compatible | 字符串模板引擎 |
| Stdlib | `@std/compression` / `@std/compress/gzip` | compatible | flate2 |
| Stdlib | `@std/terminal` | partial | 基础终端控制(光标/清屏),高级待补 |
| Stdlib | `@std/cli` | compatible | 命令行参数解析 |
| Stdlib | `@std/toml` | compatible | parse/stringify(toml crate) |
| Stdlib | `@std/yaml` | compatible | serde_yaml |
| Stdlib | `@std/xml` | compatible | quick-xml |
| Stdlib | `@std/markdown` | partial | parse/renderTerminal/fromHTML;createStream 未实现 |
| Stdlib | `@std/schema` | compatible | validate/assert(JSON-Schema 风格) |
| Stdlib | `@std/test` | partial | test/it/describe/expect/runner(宿主闭包) |
| Stdlib | `@std/archive/zip` | compatible | list/extract/create(路径穿越防护) |
| Stdlib | `@std/buffer` | compatible | from/alloc/byteLength/concat/isBuffer(utf8/hex/base64) |
| Stdlib | `@std/events` | compatible | EventEmitter on/once/off/emit/listeners |
| Stdlib | `@std/jwt` | compatible | sign/verify/decode(HS256,iat 自动注入、exp 校验) |
| Stdlib | `@std/mime` | compatible | typeByExtension/extensionByType/parseMediaType |
| Stdlib | `@std/net/ip` | compatible | parseIP/parseCIDR/contains(IPv4+IPv6) |
| Stdlib | `@std/retry` | compatible | run/exponential(指数退避) |
| Stdlib | `@std/stream` | compatible | fromString(同步只读流)+ read/readLine |
| Stdlib | `@std/exec` | compatible | run/output/combinedOutput/command |
| Stdlib | `@std/rate-limit` | compatible | create({rate,capacity}) + tryAcquire/acquire(token bucket) |
| Stdlib | `@std/prometheus` | compatible | create 工厂 + inc/set/get/snapshot |
| Stdlib | `@std/highlight` | compatible | terminal(code,{lang}) + diff/json/shell/toml 着色 |
| Stdlib | `@std/sse` | compatible | parse(text) + reader(stream) |
| Stdlib | `@std/db` | compatible | sqlite(rusqlite bundled);postgres/mysql/mssql 返回 unsupported |
| Stdlib | `@std/mail` | compatible | parseAddress/formatAddress(RFC 5322/RFC1123Z) |
| Stdlib | `@std/net/http/client` | compatible | get/post/request/fetch + **requestAsync/streamAsync**(reqwest 连接池,keep-alive) |
| Stdlib | `@std/net/http/server` | compatible | createServer(handler,port) + acceptOne(tiny_http,同步) |
| Stdlib | `@std/net/socket/{client,server}` | compatible | connect/listen/accept,同步阻塞 I/O + 读超时 |
| Stdlib | `@std/net/ws/{client,server}` | compatible | 自实现 RFC 6455 握手+帧编解码 |
| Stdlib | `@std/runtime` | compatible | runScript/callScript/runTool + mode/state |
| Stdlib | `@std/image` / `@std/pdf` | compatible | info(path) 占位(对齐 Go 版 placeholder) |
| Stdlib | `@std/async` | compatible | async 工具(依赖 Promise) |
| Stdlib | `@std/web` (含 `@std/express` 别名) | compatible | createApp + 路由 + 中间件 + **listen({workers:N}) prefork 并发** + 单 worker 异步不阻塞 + 流式/SSE |
| Stdlib | `@std/tui` | partial | 基础交互式 TUI 已实现,深度能力待补 |
| Stdlib | `@std/gtp/client` | compatible | connectTcp/connect/call/recv/close/isAlive |
| Stdlib | `@std/gtp/server` | partial | 占位 |
| GTP | sdk frame/jsonl | compatible | 与 Go 版逐字节兼容 |
| GTP | transport(stdio/tcp) | compatible | |
| GTP | scheduler/im-bot plugin | partial | PluginManager 骨架,生命周期/配置待补 |
| Typecheck | annotations parse | compatible | parser 支持 |
| Typecheck | checker | missing | P11 |
| Tests | Rust unit/integration | compatible | 31 个集成测试套件 + bytecode parity/verification/modules/async/alloc/perf,全绿 |
| Tests | Go/Rust parity runner | partial | `scripts/parity-runner.ps1`,51 fixture(49 双跑一致 + 2 Rust-only) |
| Perf | web 并发基准 | compatible | `bench/`,I/O 密集近线性扩展(workers=8 → 7.9×) |
| Docs | README / ARCHITECTURE | compatible | 已新增 |
