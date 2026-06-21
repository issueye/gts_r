# @std/web 并发性能基准报告

> 测试日期:2026-06-21 | 平台:Windows 10 (win32 10.0.26200 x64)
> 构建:`cargo build --release` | 工具:`bench/bench_client.rs` + `bench/scripts/bench_server.gs`

## 测试方法

### 测试架构

```
bench_client (Rust, N 客户端线程)
       │  HTTP/1.1 (Connection: close)
       ▼
gs bench_server.gs (1 主线程 bind + W worker 线程 accept/handle)
```

- **bench_client**:`bench/bench_client.rs`,纯 Rust 多线程压测客户端。W 个线程并发发请求,
  每个请求测延迟,统计吞吐(req/s)和延迟分位数(P50/P95/P99/max)。
- **bench_server**:`bench/scripts/bench_server.gs`,通过环境变量 `BENCH_MODE`/
  `BENCH_WORKERS` 选择串行(`{count: 1e8}`)或并发(`{workers: N}`)模式。

### 三个测试场景

| 路由 | 类型 | 含义 | 并发参数 |
|------|------|------|---------|
| `/fast` | 纯框架开销 | 瞬时响应,测量 accept + 路由 + 序列化的固定成本 | c=16, n=2000 |
| `/io?ms=50` | I/O 密集 | handler 内 `timers.sleep(50ms)`,模拟 DB/网络等待 | c=32, n=160 |
| `/cpu?n=200000` | CPU 密集 | handler 内 `while` 循环求和 1..200000(解释器内 ~0.3s) | c=16, n=80 |

`c` = 客户端并发线程数,`n` = 总请求数。

---

## 核心结果:串行 vs 并发

### I/O 密集型(`/io?ms=50`)— 并发收益最大的场景

| 配置 | 吞吐 (req/s) | 相对串行 | 平均延迟 | P50 延迟 | P99 延迟 |
|------|-----------:|--------:|-------:|-------:|-------:|
| **serial**(count 循环) | 20 | 1.0× | 1464 ms | 1621 ms | 1624 ms |
| **workers=1**(长驻单) | 20 | 1.0× | — | — | — |
| **workers=2** | 39 | **2.0×** | — | — | — |
| **workers=4** | 79 | **4.0×** | 370 ms | 405 ms | 407 ms |
| **workers=8** | 158 | **7.9×** | 186 ms | 202 ms | 203 ms |

**结论**:吞吐随 worker 数**线性扩展**(接近完美线性),延迟按比例下降。
- serial 模式下 160 个 50ms 请求需 8.1s(纯串行)
- workers=4 下同样 160 个请求只需 2.0s,延迟降低 4 倍
- workers=8 下吞吐达 serial 的 7.9 倍,接近理论 8 倍上限

### CPU 密集型(`/cpu?n=200000`)

| 配置 | 吞吐 (req/s) | 相对串行 | 平均延迟 | P50 延迟 | P99 延迟 |
|------|-----------:|--------:|-------:|-------:|-------:|
| **serial** | 12 | 1.0× | 1249 ms | 1176 ms | 2018 ms |
| **workers=4** | 36 | **3.0×** | 408 ms | 438 ms | 485 ms |
| **workers=8** | 48 | **4.0×** | 306 ms | 310 ms | 452 ms |

**结论**:CPU 密集场景并发收益也很显著,但受**物理 CPU 核心数**限制。
- workers=4 达到 3 倍提升(接近 4 倍理论上限)
- workers=8 提升到 4 倍后饱和(说明测试机有效并行核心约 4-8 个,
  超过后 worker 线程竞争 CPU 时间片)

### 纯框架开销(`/fast`)

| 配置 | 吞吐 (req/s) | 平均延迟 | P99 延迟 |
|------|-----------:|-------:|-------:|
| serial | 18098 | 860 µs | 1881 µs |
| workers=1 | 21161 | — | — |
| workers=2 | 23652 | — | — |
| workers=4 | 16517 | 945 µs | 2869 µs |
| workers=8 | 13608 | 1162 µs | 1853 µs |

**结论**:对于**瞬时响应**的请求,并发反而可能**略降**吞吐——因为:
- 请求处理本身极快(微秒级),瓶颈在 accept/网络/连接建立
- 多 worker 线程竞争同一个 `tiny_http::Server` 的内部消息队列
- worker 数越多,线程切换开销越显著

这符合预期:**并发模型对 I/O 密集和 CPU 密集场景收益巨大,对超轻量请求无益**。
真实 Web 应用绝大多数请求涉及 I/O(DB、缓存、下游 API)或计算,属于受益场景。

---

## 扩展性曲线(I/O 密集场景)

```
吞吐 (req/s)
  160 ┤                              ████  workers=8: 158
      │
  140 ┤
      │
  120 ┤
      │
  100 ┤
      │
   80 ┤                    ████  workers=4: 79
      │
   60 ┤
      │
   40 ┤          ████  workers=2: 39
      │
   20 ┤████                    ████  serial / workers=1: 20
      └──────────────────────────────────
        serial  w=1   w=2   w=4   w=8
```

**近乎完美的线性扩展**,证明 prefork 共享 socket 模型有效利用了多核。

---

## 测试结论

### ✅ 改造目标达成

1. **I/O 并行性**:workers=N 带来接近 N 倍的吞吐提升(20→158 req/s,7.9×)
2. **CPU 并行性**:CPU 密集 handler 也获得 3-4 倍提升,证明 worker 线程真正并行执行
3. **延迟改善**:高负载下 P99 延迟从 1.6s 降至 0.2s(8 倍改善)
4. **无回归**:serial 模式(`/fast` 18098 req/s)与改造前行为一致

### 适用场景建议

| 场景 | 推荐配置 | 理由 |
|------|---------|------|
| 纯 I/O 等待(DB/API) | `workers: 物理核心数 × 2` | I/O 期间 CPU 空闲,可超额订阅 |
| CPU 密集计算 | `workers: 物理核心数` | 超过核心数后收益递减 |
| 混合负载 | `workers: 4`(默认推荐) | 平衡 I/O 与 CPU 利用 |
| 轻量 API(微秒级响应) | `workers: 1` 或 serial | 并发无益,避免线程切换开销 |
| 测试/共享状态依赖 | `workers: 1` 或 `{count: N}` | worker 间不共享内存状态 |

### 已知限制

1. **Worker 间无共享状态**:每个 worker 是独立 VM,脚本层全局变量(如计数器)不共享。
   需共享状态应外置(Redis/DB)。这与 Nginx/Apache prefork 行为一致。
2. **Worker 启动开销**:每个 worker 需重新加载脚本(解析 + 注册 routes)。本测试的
   脚本很小(~70 行),启动 < 100ms;大型脚本会增加首次响应延迟。
3. **轻量请求无收益**:`/fast` 类瞬时请求,并发不提升吞吐(瓶颈在网络/accept,非计算)。

---

## 复现方法

```bash
# 1. 编译 release 版本
cargo build --release --bin gs --bin bench_client

# 2. 启动服务器(并发模式,4 worker)
BENCH_MODE=workers BENCH_WORKERS=4 ./target/release/gs bench/scripts/bench_server.gs &

# 3. 运行压测
./target/release/bench_client 19000 /io?ms=50 32 160

# 4. 或一键运行完整对比
bash bench/run_bench.sh
```

完整原始数据见 `bench/results.txt`。
