# @std/web 并发服务器设计文档

## 概述

本文档描述 `@std/web` (`@std/express`) 框架的并发服务器实现。改造前,
`app.listen(port, {count: N})` 在 VM 主线程**串行**处理 N 个请求后销毁
服务器返回,无法支撑并发负载。改造后,`app.listen(port, {workers: N})`
通过 **prefork 共享 socket** 模型让多个 worker 线程并行处理请求。

## 核心约束与设计决策

### 硬约束:VM 单线程,Object 非 Send

GTS 是单线程 tree-walking 解释器。`Object` 枚举的 13 个变体中,10 个用
`Rc<RefCell<T>>`(非 `Send`);`Environment` 持有 `Rc<VirtualMachine>`;
`BuiltinFn` 是 `Rc<dyn Fn>`。

**用户 handler 函数对象无法跨线程传递** —— 这是类型系统层面的限制。
绕开的唯一代价是把整个 Object 系统从 `Rc` 改成 `Arc`、`RefCell` 改成
`Mutex`/`RwLock`,属于 VM 级重构(约 400 处改动点,遍布 20 个源文件)。
项目文档 `TOKIO_INTEGRATION.md`、`AWAITABLE_BRIDGE_SUMMARY.md` 已明确
拒绝此路线,以保持 native 求值路径零开销。

### 选定方案:Prefork 共享 Socket

不碰 Object 内核,通过 **每个 worker 线程持有独立 VM 实例**(各自
`Session::new()` + 重新加载脚本 + 注册 routes)绕开非 Send 问题。

`tiny_http::Server` 本身是 `Send + Sync`(源码 `lib.rs:164` 通过
`MustBeShareDummy` trait 强制),其 `recv()`/`recv_timeout()` 通过 `&self`
调用。多个线程可共享同一个 `Arc<Server>` 并发调用 `recv_timeout()`,由
OS 内核决定哪个线程拿到连接 —— 标准的 accept-ready 模型,Windows/
Linux/macOS 通用,无需真正的 fork。

## 架构

```
主线程 (main VM)
  │
  ├─ app.listen(port, { workers: N }) 被调用
  │     │
  │     ├─ tiny_http::Server::http("0.0.0.0:port")  绑定
  │     ├─ Arc<Server> + Arc<AtomicBool shutdown>   共享原语
  │     ├─ 发布到 WebApp (供 app.close() 触发)
  │     │
  │     ├─ spawn worker 线程 0..N
  │     │     每个 worker:
  │     │       ├─ 设置 thread_local WEB_WORKER_CTX {server, shutdown, id}
  │     │       ├─ Session::new()  独立 VM
  │     │       ├─ run_source(脚本)  重建 routes
  │     │       │     └─ 脚本顶层 app.listen() 被拦截 → web_listen_worker()
  │     │       └─ accept 循环: recv_timeout(100ms) → web_handle_request()
  │     │
  │     ├─ 安装 Ctrl+C handler (unix: SIGINT)
  │     └─ join 所有 worker
  │
  └─ 返回
```

### Worker 路由重建的关键机制

worker 线程重新执行用户脚本顶层时,`app.listen(...)` 会**再次被调用**。
如果不处理,会导致无限递归 spawn。

**解法**:thread_local 标志 `WEB_WORKER_CTX`。主线程 spawn worker 前,
worker 设置该 thread_local。worker 内重新执行脚本到达 `app.listen()` 时,
`web_listen` 函数检测到 thread_local 已设置,**跳过** bind 和 spawn,直接
进入 `web_listen_worker` 的 accept 循环。这样:

- 每个 worker 拥有自己独立构建的 `WebApp`(独立 routes 副本)
- handler 闭包的 `f.env` 作用域链完全独立
- 没有 Object 跨线程,只有 `Arc<tiny_http::Server>` 共享

### 脚本路径获取

worker 需要重新加载脚本。`VirtualMachine.bootstrap_source` 字段原本闲置,
现在在 `run_source_with_options` 中被填充为入口脚本绝对路径。worker 从
主线程 VM 的 `bootstrap_source` 读取路径,`fs::read_to_string` 加载源码。

## API 演进

```javascript
// 串行模式(原有,默认 count: 1,用于测试)
app.listen(port, { count: 1 });

// 并发模式(新)—— N 线程并行处理请求,长驻直到关闭
app.listen(port, { workers: 4 });

// 长驻单 worker —— 串行语义但阻塞直到关闭(非 count 限定)
app.listen(port, { workers: 1 });

// workers 优先于 count
app.listen(port, { workers: 4, count: 10 }); // 走 workers 路径

// 显式关闭(让 listen 返回)
app.close();
```

### CLI 支持

```bash
# --workers 作为全局默认,通过环境变量 GTS_DEFAULT_WORKERS 暴露给脚本
gs --workers 4 main.gs
```

脚本侧可通过 `process.env.GTS_DEFAULT_WORKERS` 读取,但 `app.listen` 的
`{ workers: N }` 选项总是优先。

## 共享状态语义

**重要**:prefork 模型下,worker 间**不共享**脚本层的可变状态。例如:

```javascript
let counter = 0;  // 每个 worker 各有独立副本
app.get("/inc", function(req, res) {
    counter++;              // 只在当前 worker 内递增
    res.send(String(counter));
});
app.listen(port, { workers: 4 });
```

4 个 worker 各自维护独立的 `counter`,不同请求命中不同 worker 会看到不同值。
这与真实 prefork 服务器(Nginx、Apache、Node cluster)行为一致 —— 跨 worker
共享状态应外置(Redis、共享数据库、消息队列)。

对于 `workers: 1`(单 worker 长驻),语义与串行模式等价,共享状态可见。

## app.close() 的跨 worker 语义

`app.close()` 可在两个位置调用:

1. **主线程**(listen 之外的代码):设置 `WebApp.shutdown_signal` +
   `shared_server.unblock()`,唤醒所有 worker。
2. **worker 内的 handler**:worker 的 `WebApp` 是独立实例,其
   `shutdown_signal` 为 `None`。此时通过 `WEB_WORKER_CTX` thread_local
   拿到**共享的** shutdown flag,设置它 + `server.unblock()`。所有 worker
   在下一次 `recv_timeout` 轮询时检测到 flag,退出 accept 循环。

## 关键实现文件

| 文件 | 改动 |
|------|------|
| `src/stdlib/mod.rs` | `WebApp` 新增 `shared_server`/`shutdown_signal` 字段;新增 `web_listen_serial`/`web_listen_worker`/`web_listen_concurrent`/`ctrlc_set_flag`;`web_listen` 路由分发;`close` 闭包支持 worker 线程;响应加 `Connection: close` 头 |
| `src/runtime/mod.rs` | `run_source_with_options` 填充 `bootstrap_source`(脚本路径) |
| `src/bin/gs.rs` | 激活 `--workers` CLI flag,透传给 `run_script` 并设为 `GTS_DEFAULT_WORKERS` 环境变量 |
| `tests/stdlib_p9_web.rs` | 新增 3 个并发测试:`web_concurrent_requests_run_in_parallel`、`web_workers_serve_multiple_routes`、`web_close_shuts_down_workers` |

## 验收测试

1. **并行性证明**:`web_concurrent_requests_run_in_parallel` —— 4 个 300ms
   慢请求在 < 800ms 内完成(串行需 1200ms+)。
2. **多路由**:`web_workers_serve_multiple_routes` —— 并发命中不同路由均
   正确响应。
3. **优雅关闭**:`web_close_shuts_down_workers` —— handler 内 `app.close()`
   让长驻服务器进程在 5 秒内退出。
4. **回归保证**:原有 5 个 `count: N` 串行测试 + 4 个 http_server 测试全部
   通过,串行路径行为完全不变。

## 不在本次改造范围

- Object 系统的 `Rc → Arc`(VM 级重构,项目已拒绝)
- 激活 `EventLoop`/`awaitable` 的真异步(独立大工程)
- WebSocket 服务器的并发改造(可复用本方案的 thread_local worker 模式)
- 跨 worker 共享状态的同步机制(需外置,非本层职责)
- `@std/net/http/server` 模块的并发化(结构相同,可后续应用相同模式)
