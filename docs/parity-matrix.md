# GoScript Rust Parity Matrix

> 状态枚举：`missing` 未实现；`partial` 部分实现；`compatible` 已按 Go 版行为验证；`blocked` 需要设计/依赖；`deprecated` Go 版已废弃或明确不复刻。

| Area | Item | Status | Notes |
|---|---|---:|---|
| CLI | direct file execution | partial | Rust 可运行 `.gs` 文件，需补 flags/错误输出/main 自动调用 |
| CLI | `run` project entry | partial | 简易 `project.toml entry` 支持，需迁移 Go `internal/proj` |
| CLI | `run-script` | compatible | `gs run-script <script.gs> [args...]` 自动调用 main 函数 |
| CLI | `init` | compatible | 已实现，`tests/cli_flags.rs` 验证脚手架与 project.toml |
| CLI | `pack` | missing | 依赖 `.gspkg` |
| CLI | `dist` | missing | 依赖 packagefile append |
| CLI | `bundle` | missing | 依赖 bundler |
| CLI | REPL | compatible | 支持 `.help/.exit` 与持久会话，`cli_flags.rs` 验证 |
| CLI | LSP | missing | Go 版 `internal/lsp` |
| CLI | `--timeout` | compatible | 能中止死循环，`cli_timeout_stops_infinite_loop` 验证 |
| CLI | `--workers` | partial | flag 已解析并透传 RunOptions，worker 调度语义未对齐 |
| CLI | `--check-types` | missing | 占位返回 not-implemented |
| Language | lexer | partial | 已迁移，需 Go 测试对照 |
| Language | parser | partial | 已迁移，需完整语法对照 |
| Language | evaluator core | partial | 基础运行可用，需语义双跑 |
| Language | classes/inheritance | partial | 代码存在，需测试对照 |
| Language | match | compatible | 基础值匹配、OR 模式、变量绑定、守卫条件，`lang_match.rs` 验证 |
| Language | errors/stack | partial | 需对齐 Error 子类与 stack |
| Async | Promise | compatible | then/catch/finally/all/race 已实现，`async_promise.rs` 验证 |
| Async | timers | partial | 需验证 setTimeout/setInterval/sleepAsync |
| Modules | relative `require` | partial | 基础支持，需 cache/circular/module.exports 对齐 |
| Modules | ES import/export | partial | evaluator 有基础，loader 简化 |
| Modules | native `@std/*` registry | partial | 已接入 45 个模块（见下方 Stdlib 明细） |
| Modules | package resolver | missing | Go `internal/module/resolver.go` |
| Packaging | `.gspkg` | missing | Go packagefile |
| Packaging | nested package | missing | examples/14 |
| Packaging | executable embedding | missing | `dist` |
| Stdlib | core globals | partial | Math/JSON/Object/Array/String/Number/Boolean 等部分 |
| Stdlib | `@std/fs` | compatible | 已含 enhanced API（walkSync/globSync/createThrottledWriter…），`cli_flags.rs` 验证 |
| Stdlib | `@std/path` | compatible | `cli_flags.rs` 验证 |
| Stdlib | `@std/os` | partial | 已迁移，需 Go 行为对照 |
| Stdlib | `@std/env` | compatible | 16 函数（get/getInt/getFloat/getBool/getArray/getJson/has/require/set/unset/toObject/load/loadMultiple/parse + getString/getNumber 别名），require 支持数组，`stdlib_p6b.rs` 验证 |
| Stdlib | `@std/json` | compatible | parse5/validate/get/set/patch/diff，`cli_flags.rs` 验证 |
| Stdlib | `@std/time` | compatible | parse/format/add/parseDuration/unix/sleep，`cli_flags.rs` 验证 |
| Stdlib | `@std/encoding/base64` | compatible | encode/decode/encodeURL/decodeURL，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/encoding/hex` | compatible | encode/decode，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/hash` | compatible | adler32/crc32(IEEE)/crc64(ISO)/fnv1a 已知向量核对，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/random` | compatible | int/float/bool/pick/sample/shuffle/hex/base64/alphanumeric/alpha/numeric/uuid/bytes，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/regexp` | compatible | escape/matchAll/split（RE2，无 flags），`stdlib_p6.rs` 验证 |
| Stdlib | `@std/semver` | compatible | parse/compare/gt/lt/eq/inc/satisfies(^/~/>/</>=/<=/=)，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/collections` | compatible | unique/chunk/flatten/sample/shuffle/range，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/process` | compatible | argv/pid/version/cwd/getenv/hrtime/…，`stdlib_p6.rs` 验证 |
| Stdlib | `@std/timers` | partial | 转发层（setTimeout/setInterval/sleepAsync）+ sleep/clear*/queueMicrotask；同步单线程 VM 下 clear* 为 no-op，`stdlib_p6b.rs` 验证 sleep 与转发 |
| Stdlib | `@std/cache` | compatible | create 工厂 + set/get/has/delete/clear/size/keys，毫秒 TTL、惰性过期，`stdlib_p6b.rs` 验证 |
| Stdlib | `@std/text` | compatible | chars/runes/width/truncateWidth/padRightWidth/wrapWidth/stripAnsi（显示宽度，CJK=2、组合符=0），`stdlib_p6b.rs` 验证 |
| Stdlib | `@std/url` | compatible | parse/format/resolve/pathToFileURL/fileURLToPath（origin 对相对 URL 返回 "null"），`stdlib_p6b.rs` 验证 |
| Stdlib | `@std/crypto` | compatible | sha1/256/512（NIST 向量核对）/hmac（RFC 4231）/pbkdf2（RFC 向量核对）/randomUUID/randomBytes/timingSafeEqual，`stdlib_p6b.rs` 验证 |
| Stdlib | `@std/glob` | compatible | glob 通配匹配，复用现有 glob_paths，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/color` | compatible | ANSI 颜色输出，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/diff` | compatible | 文本/对象 diff，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/log` | compatible | 结构化日志，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/table` | compatible | ASCII 表格渲染，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/validation` | compatible | 校验器，`stdlib_p7.rs` 验证 |
| Stdlib | `@std/encoding/csv` | compatible | parse/stringify/readFileSync/writeFileSync，`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/template` | compatible | 字符串模板引擎，`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/compression` | compatible | 通用压缩接口，`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/compress/gzip` | compatible | gzip（flate2），`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/terminal` | partial | 终端控制（光标/清屏），`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/cli` | compatible | 命令行参数解析，`stdlib_p7b.rs` 验证 |
| Stdlib | `@std/toml` | compatible | parse/stringify/readFileSync/writeFileSync（toml crate），`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/yaml` | compatible | parse/stringify/readFileSync/writeFileSync（serde_yaml），`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/xml` | compatible | parse/stringify/readFileSync/writeFileSync（quick-xml），DOM 节点 name/attributes/children/text，自闭合规则，`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/markdown` | partial | parse/renderTerminal/fromHTML（无 markdown→HTML render，对齐 Go 版）；createStream 未实现，`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/schema` | compatible | validate/assert（JSON-Schema 风格：type/enum/required/properties/minItems/items/minLength/minimum），`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/test` | partial | test/it/describe/expect(toBe/toEqual/toTruthy/toFalsy)/run runner（宿主可调脚本闭包，对齐 Go 版占位），`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/archive/zip` | compatible | list/extract/create（zip crate，路径穿越防护），`stdlib_p7c.rs` 验证 |
| Stdlib | `@std/buffer` | compatible | from/alloc/byteLength/concat/isBuffer（utf8/hex/base64），`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/events` | compatible | EventEmitter on/once/off/emit(同步)/listeners/listenerCount/removeAllListeners，`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/jwt` | compatible | sign/verify/decode（HS256，自包含 hmac+sha256，iat 自动注入、exp 校验），`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/mime` | compatible | typeByExtension/extensionByType/parseMediaType/formatMediaType（内置 MIME 表），`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/net/ip` | compatible | parseIP/parseCIDR/contains/splitHostPort/joinHostPort/lookupHost（IPv4+IPv6、CIDR 包含判定），`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/retry` | compatible | run/exponential（同步阻塞、可配置 times/delay/backoff、指数退避），`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/stream` | compatible | fromString（同步只读流）+ read/readText/readLine/readAll/close，`stdlib_p7d.rs` 验证 |
| Stdlib | `@std/exec` | compatible | run/output/combinedOutput/command，`stdlib_p8_exec.rs` 验证 |
| Stdlib | `@std/net/http/client` | compatible | get/post/request/fetch，`stdlib_p8_http.rs` 验证 |
| Stdlib | `@std/rate-limit` | compatible | create({rate,capacity}) + tryAcquire/acquire/remaining，token bucket；同步 VM 下 acquire 阻塞 sleep，`stdlib_p9.rs` 验证 |
| Stdlib | `@std/prometheus` | compatible | create 工厂 + inc/set/get/snapshot，`stdlib_p9.rs` 验证 |
| Stdlib | `@std/highlight` | compatible | terminal(code,{lang,width,color})，diff/json/shell/toml 注释着色、ANSI 转义，`stdlib_p9.rs` 验证 |
| Stdlib | `@std/sse` | compatible | parse(text) + reader(streamOrText)，next/readAll 游标遍历、多 data 行合并，`stdlib_p9.rs` 验证 |
| Stdlib | `@std/db` | compatible | open/drivers + exec/query/queryOne/prepare/begin/commit/rollback/ping/close（sqlite，rusqlite bundled），postgres/mysql/mssql 仅返回 unsupported 错误（Rust 端只复刻 sqlite），`stdlib_p9_db.rs` 验证 |
| Stdlib | `@std/mail` | compatible | parseAddress/parseAddressList/parseMessage/formatAddress/formatAddressList/parseDate/formatDate/getHeader（RFC 5322 地址解析、邮件头/体拆分、RFC1123Z 格式化），`stdlib_p9_mail.rs` 验证 |
| Stdlib | `@std/net/socket/client` | partial | connect/dial + write/send/read/recv/close/setDeadline（同步阻塞 I/O、DNS 解析、读超时），同步 VM 下 connect 超时 30s；`stdlib_p9_socket.rs` 验证；Go 版无超时差异 |
| Stdlib | `@std/net/socket/server` | partial | listen/createServer + acceptOne/accept/close；**无后台事件循环**，acceptOne 同步阻塞单连接（非阻塞 listener + WouldBlock 哨兵），handler 可在 listen 注册或 acceptOne 显式传入；`stdlib_p9_socket.rs` 验证 echo 回环 |
| Stdlib | `@std/runtime` | compatible | runScript(path,{argv,autoMain})/callScript(path,name,args,opts)/runTool(path,input,opts)；每次新建独立 Session（隔离 VM/argv），返回 module.exports 或导出函数调用结果；`stdlib_p9_runtime.rs` 验证 |
| Stdlib | `@std/image` | compatible | info(path) 占位（对齐 Go 版 placeholder，返回 "requires external library" 错误），`stdlib_p9_runtime.rs` 验证 |
| Stdlib | `@std/pdf` | compatible | info(path) 占位（对齐 Go 版 placeholder，返回 "requires external library" 错误），`stdlib_p9_runtime.rs` 验证 |
| Stdlib | `@std/net/ws/client` | partial | connect(url,headers?)；自实现 RFC 6455 握手+帧编解码（复用内置 sha1/base64），send/sendText/sendBinary/recv/close；阻塞 I/O；`stdlib_p9_ws.rs` 验证（含真实 Rust echo 服务器回环） |
| Stdlib | `@std/net/ws/server` | partial | createServer(port,handler?)/upgrade；**无后台事件循环**，acceptOne 同步完成握手+调用 handler（非阻塞 listener + WouldBlock 哨兵）；upgrade 在同步运行时不可用（无 HTTP hijack 目标），返回明确错误指向 createServer.acceptOne；`stdlib_p9_ws.rs` 验证 |
| Stdlib | `@std/net/http/server` | partial | createServer(handler?,port?) + acceptOne/accept/close（tiny_http，同步）；请求对象 {method,url,path,body,query,headers,remoteAddr}、响应对象 {status,setHeader,send,json,end}；query 百分号解码、Content-Type 默认 text/plain、json 自动 application/json；handler 抛错返回 500；`stdlib_p9_http_server.rs` 验证（GET/JSON+status/query+body/自定义 header 四类真实 HTTP 回环） |
| Stdlib | `@std/web` (含 `@std/express` 别名) | partial | createApp() + get/post/put/patch/delete/all(path,handler) + use([path],handler) 中间件 + listen(port,{count});路径参数 `/users/:id`、前缀匹配中间件、ctx={req,res,params}、无匹配自动 404、handler 抛错 500；**无后台事件循环**，listen 处理 count 个请求后返回；`stdlib_p9_web.rs` 验证（GET/params+json/404/middleware 四类真实回环 + web.json 序列化） |
| Stdlib | network/web/db modules | partial | P8，http/client、http/server、exec、db(sqlite)、rate-limit、prometheus、highlight、sse、mail、net/socket/{client,server}、net/ws/{client,server}、runtime、web 已完成 |
| Stdlib | tui/terminal advanced | missing | P9 |
| GTP | sdk frame/jsonl | missing | P10 |
| GTP | scheduler plugin | missing | P10 |
| GTP | im-bot plugin | missing | P10 |
| Typecheck | annotations parse | partial | parser 支持，checker 未实现 |
| Typecheck | checker | missing | P11 |
| Tests | Rust unit/integration | partial | 174 用例（13 cli + 2 parity + 10 runtime + 23 stdlib_p6 + 19 stdlib_p6b + 7 stdlib_p7 + 6 stdlib_p7b + 13 stdlib_p7c + 12 stdlib_p7d + 6 stdlib_p8_exec + 4 stdlib_p8_http + 9 stdlib_p9 + 5 stdlib_p9_db + 8 stdlib_p9_mail + 4 stdlib_p9_socket + 6 stdlib_p9_runtime + 6 stdlib_p9_ws + 4 stdlib_p9_http_server + 5 stdlib_p9_web），全绿 |
| Tests | Go/Rust parity runner | missing | P0 首要任务，仅 Rust 侧 fixture 双跑 |
| Docs | Rust README/status | missing | P12 |
