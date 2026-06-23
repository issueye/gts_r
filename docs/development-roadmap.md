# GoScript (Rust) 开发路线图

> 本路线图反映 2026-06-23 的实际代码状态。历史版本描述了大量"缺失功能"(Date/Map/Set/网络模块/字节码 VM),这些**已全部实现**——本文以代码为准重新校准。
>
> 状态快照:字节码 VM 已全量交付并设为默认后端(阶段 0–11 完成);68 个 `@std/*` 模块已接入;Tokio I/O 默认启用,单 worker 并发与流式代理/SSE 已完成;31 个集成测试套件全绿。

---

## 1. 已完成的里程碑

### 1.1 执行后端:字节码 VM(默认)

树遍历求值器已迁移到栈式字节码 VM。详见 [`bytecode-vm-development-plan.md`](bytecode-vm-development-plan.md) 与 [`bytecode-vm-todo.md`](bytecode-vm-todo.md)。

- ✅ 全量 AST 覆盖:18 种 Stmt + 27 种 Expr + Match 模式系统 + 类系统 + 模块系统 + 类型注解
- ✅ VM 单跑全绿:parity fixture + verification 套件 + modules + async 套件
- ✅ 性能不劣于树遍历(fib / 字符串拼接 / Promise 创建三类基准)
- ✅ `Session::new()` / `VirtualMachine::new()` 默认 Bytecode;`--exec-mode=tree` 保留 legacy fallback
- ✅ 闭包与 upvalue(开放/闭合两态)、native↔VM 互调桥接、`arguments` 对象、spread、super 分派

### 1.2 异步运行时

- ✅ Native runtime:`EventLoop` + `TimerWheel` + `IoSelector`(epoll/kqueue/IOCP/poll 跨平台多路复用)
- ✅ Tokio runtime(默认 feature):多线程 I/O,`Session::new()` 自动具备
- ✅ Completion queue 桥接:VM 线程 drain completion,Tokio 投递 owned `Send` 结果,Promise 在 VM 线程 resolve
- ✅ 异步 HTTP client:`http.requestAsync` / `http.streamAsync`(reqwest 连接池,keep-alive)

### 1.3 `@std/web` 并发

- ✅ prefork 共享 socket:`app.listen(port, { workers: N })` 多 worker 并行
- ✅ 单 worker 异步不阻塞:handler 返回 Promise 时挂起响应、继续 accept
- ✅ 流式响应 / SSE:res.write / res.stream,chunked 转发
- ✅ handler 签名收敛为 `(req, res, next)`;gs-llm-bridge 代理链路已切到 `await http.requestAsync`
- ✅ 性能基准:I/O 密集近线性扩展(workers=8 → 7.9×),详见 [`web-concurrency-benchmark.md`](web-concurrency-benchmark.md)

### 1.4 标准库(68 个 `@std/*` 模块)

全部 68 个模块已接入并测试,涵盖 fs/path/os/process/exec/crypto/hash/random/time/timers/json/yaml/toml/xml/markdown/template/schema/validation/log/table/color/diff/glob/compression/gzip/archive-zip/buffer/events/jwt/mime/retry/stream/rate-limit/prometheus/highlight/sse/db/mail/net-ip/net-http-{client,server}/net-socket-{client,server}/net-ws-{client,server}/web/express/runtime/image/pdf/tui/terminal/cli/env/text/url/cache/collections/semver/regexp/encoding-{base64,hex,csv}/async/gtp。逐项状态见 [`parity-matrix.md`](parity-matrix.md)。

### 1.5 GTP 协议

- ✅ Frame / Value / GtpError(JSON Lines,与 Go 版逐字节兼容)
- ✅ Transport trait + stdio / TCP 传输
- ✅ `@std/gtp/client`(`connectTcp` / `call` / `recv` / `close` / `isAlive`)
- ✅ `@std/gtp/server` 占位

### 1.6 语言核心

- ✅ 类与继承(构造器 / super / 静态成员 / 字段)、闭包、async/await、Promise.all/race/allSettled、模式匹配(literal/ident/wildcard/or/range + guard)、try/catch/finally、Map/Set/Date 全套方法

---

## 2. 当前缺口(按优先级)

### 🔴 高优先 — 产品完整度与兼容性

| 缺口 | 影响 | 备注 |
|------|------|------|
| Go/Rust 双跑 fixture 扩容 | 已有 51 个 parity fixture;仍需覆盖更多语法边界、错误路径 | `scripts/parity-runner.ps1` |
| tree fallback 下线 | VM 已默认,树遍历仍保留;部分语法(如 `??` 已补)曾是 fallback-only | 见 `bytecode-vm-todo.md` 阶段 11 |
| `--workers` 真实 worker 池 | CLI flag 已透传,`@std/web` 已用 prefork;其他场景调度语义待统一 | |
| package resolver + `.gspkg` 完整 | `gs init/pack/dist/bundle` 部分实现;多文件项目分发链待完善 | |
| ES `import/export` 完整语义 | named/default/namespace/re-export 不完整 | |

### 🟡 中优先 — 增强与对齐

| 缺口 | 影响 |
|------|------|
| `--check-types` + typechecker | 占位返回 not-implemented |
| LSP | 完全缺失 |
| GTP plugin 管理器完整实现 | 骨架已有,进程生命周期 / 配置加载待补 |
| `@std/gtp/server` 实现 | 占位 |
| TUI 高级能力 | 基础已实现,深度能力待补 |

### 🟢 低优先 — 认证与文档

| 缺口 | 影响 |
|------|------|
| README / 文档对齐 | 已新增 README + ARCHITECTURE |
| CI 流水线(fmt/clippy/test/parity) | 自动化 |
| 性能基线 benchmark 扩展 | 已有 web 基准,可扩展 |

---

## 3. 活跃方向(进行中)

### 3.1 Tokio 单 worker 并发深化

详见 [`tokio-single-worker-concurrency-plan.md`](tokio-single-worker-concurrency-plan.md) 与对应 TODO。阶段 0–6 已完成(runtime 入口统一、completion queue、异步 HTTP client、Web handler Promise 化、gs-llm-bridge 切换、流式代理与 SSE)。**当前指针**:阶段 7 待拆分——真实 chunk flush / 长连接生命周期 / 更高并发压测。

### 3.2 字节码 VM tree fallback 下线

`bytecode-vm-todo.md` 阶段 11 已完成 `??` nullish coalescing 进入 VM、默认后端下沉到 `VirtualMachine::new()`。下一步:继续盘点 fallback-only 语法,规划树遍历下线 PR(需先补齐已知缺口 + CLI 迁移说明 + 独立验收门)。

---

## 4. 架构与工程约定(迁移/扩展时遵循)

### 4.1 stdlib 模块实现范式

参考 `src/stdlib/modules/` 既有模式(每个原生库一个文件):

```rust
fn xxx_module() -> Object {
    module(vec![
        ("fnName", native("xxx.fnName", xxx_fn_name)),
        // ...
    ])
}

fn xxx_fn_name(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "xxx.fnName", args, 0, "value") {
        Ok(v) => v, Err(e) => return e,
    };
    // 返回 str_obj / num_obj / bool_obj / array(...) / Object::Hash(...)
}
```

约定:
- 在 `load_native_module` 的 match 里注册新模块。
- 错误用 `new_error(ctx.pos.clone(), "模块.函数: 消息")`,消息前缀与 Go 版完全一致。
- 参数校验复用 `required_string` / `required_number` / `bytes_from_object`。
- 字节输入(base64/hex/crypto)支持 String/Array<Number>/Buffer 三态。
- 随机源用 `fill_random`(OS RNG,Win=BCryptGenRandom,Unix=/dev/urandom)。

### 4.2 测试范式

参考 `tests/stdlib_p*.rs`:写 `.gs` 到临时目录 → 跑 `gs` 二进制 → 断言 stdout/stderr/exit。跨 Go/Rust 语义通过 `scripts/parity-runner.ps1` 确认。

### 4.3 并发改造的铁律

- `Object` 非 `Send`,**绝不**跨线程传 Object/EnvRef/CallContext。
- Tokio 只做可 `Send` 的 I/O,结果通过 completion queue 回填 VM 线程。
- `Rc → Arc` 的 VM 级重构已明确拒绝。

### 4.4 依赖管理

- 已引入的成熟 crate:regex / serde / serde_json / toml / serde_yaml / quick-xml / zip / flate2 / rusqlite / tiny_http / ureq / reqwest / tokio / chrono / crossterm。
- 每个新依赖需在 PR 说明理由,避免膨胀。

---

## 5. 风险登记

| 风险 | 等级 | 缓解 |
|------|------|------|
| tree fallback 与 VM 行为细微差异 | 中 | parity fixture 逐字节比对;下线前独立验收门 |
| Go/Rust try/catch / 错误输出差异 | 中高 | parity runner 固化差异,决定对齐或记录兼容策略 |
| 网络/DB 测试 flaky | 中 | 本地端口 + 超时;CI 隔离 |
| Windows 端口耗尽(10048) | 中 | 连接池 keep-alive;单 worker 异步不阻塞 |
| TUI/PDF 跨平台 | 中 | 人工验收清单 |

---

*本文档随进度更新。每完成一个方向,更新对应小节状态与验收证据。活跃方向以对应的 `-todo.md` 追踪表为唯一真相源。*
