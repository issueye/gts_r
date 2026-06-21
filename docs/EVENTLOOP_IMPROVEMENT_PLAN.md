# EventLoop 完善和 Tokio 集成准备方案

## 一、当前状态分析

### 现有实现（src/object/）

#### awaitable.rs - 115 行
**优点**：
- ✅ 清晰的 Awaitable trait 抽象
- ✅ Poll-based 设计（类似 Rust Future）
- ✅ WakerRegistry 支持多 waker
- ✅ 单线程友好（Rc/RefCell）

**问题**：
- ❌ 无 I/O 支持（只有 Promise 和 Timer）
- ❌ 与网络 I/O 未集成
- ❌ 无超时机制

#### event_loop.rs - 169 行
**优点**：
- ✅ 任务调度和轮询
- ✅ spawn() 和 run() API
- ✅ Waker 唤醒机制

**问题**：
- ❌ **空队列时 sleep(1ms)**（效率低）
- ❌ 无 I/O 多路复用（mio/epoll）
- ❌ 无定时器堆优化
- ❌ 无优先级队列

#### timer_wheel.rs - 168 行
**优点**：
- ✅ 时间轮算法
- ✅ sleepAsync() 实现

**问题**：
- ❌ 每次轮询检查所有定时器
- ❌ 未与 EventLoop 深度集成

### 当前架构限制

```
┌─────────────────────────────────────┐
│  Script (async/await)               │
├─────────────────────────────────────┤
│  Promise / Timer                    │  ← 仅支持这些
├─────────────────────────────────────┤
│  EventLoop (task polling)           │  ← 空队列时 sleep(1ms)
├─────────────────────────────────────┤
│  ❌ 无 I/O 层                        │  ← **缺失**
└─────────────────────────────────────┘
```

**核心问题**：
- 无法等待网络 I/O（TCP/UDP/WebSocket）
- 无法等待文件 I/O
- GTP 插件通信仍是同步阻塞的

---

## 二、改进目标

### 短期目标（当前架构内）
1. ✅ 优化空队列等待（使用最近定时器）
2. ✅ 添加 I/O Awaitable 抽象
3. ✅ 集成网络 I/O 到 EventLoop
4. ✅ 实现 asyncConnect/asyncRead/asyncWrite

### 长期目标（Tokio 集成）
1. ⏳ 设计 Runtime trait 抽象
2. ⏳ 实现 NativeRuntime（当前单线程）
3. ⏳ 实现 TokioRuntime（多线程）
4. ⏳ 透明切换运行时

---

## 三、短期改进方案

### 3.1 优化 EventLoop（不依赖外部库）

#### 问题：空队列时 sleep(1ms) 效率低

**当前代码**：
```rust
if let Some(task_rc) = task {
    self.poll_task(task_rc);
} else {
    // 🔴 空队列就 sleep，即使有定时器即将到期
    std::thread::sleep(std::time::Duration::from_millis(1));
}
```

**改进方案**：
```rust
pub struct EventLoop {
    ready_queue: RefCell<VecDeque<Rc<RefCell<Task>>>>,
    timer_wheel: Rc<RefCell<TimerWheel>>,  // 新增
}

impl EventLoop {
    fn run_until_complete(...) {
        while result.borrow().is_none() {
            let task = self.ready_queue.borrow_mut().pop_front();
            
            if let Some(task_rc) = task {
                self.poll_task(task_rc);
            } else {
                // ✅ 计算下一个定时器到期时间
                let next_timer = self.timer_wheel.borrow().next_deadline();
                
                match next_timer {
                    Some(deadline) => {
                        let now = Instant::now();
                        if deadline > now {
                            // 睡眠到下一个定时器
                            std::thread::sleep(deadline - now);
                        }
                        // 触发到期的定时器
                        self.timer_wheel.borrow_mut().tick();
                    }
                    None => {
                        // 无定时器，短暂睡眠避免忙等
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
    }
}
```

### 3.2 添加 I/O Awaitable

#### 新增 io_awaitable.rs

```rust
use std::io;
use std::net::TcpStream;
use std::time::Duration;

/// I/O 操作的 Awaitable
pub struct IoAwaitable {
    inner: Rc<RefCell<IoAwaitableInner>>,
}

enum IoAwaitableInner {
    TcpConnect {
        addr: String,
        state: ConnectState,
    },
    TcpRead {
        stream: TcpStream,
        buf: Vec<u8>,
        state: ReadState,
    },
    TcpWrite {
        stream: TcpStream,
        data: Vec<u8>,
        state: WriteState,
    },
}

enum ConnectState {
    Pending,
    Ready(io::Result<TcpStream>),
}

impl Awaitable for IoAwaitable {
    fn poll(&self, waker: Waker) -> PollResult {
        let mut inner = self.inner.borrow_mut();
        
        match &mut *inner {
            IoAwaitableInner::TcpConnect { addr, state } => {
                match state {
                    ConnectState::Pending => {
                        // 非阻塞连接
                        match TcpStream::connect_timeout(
                            &addr.parse().unwrap(),
                            Duration::from_millis(0)
                        ) {
                            Ok(stream) => {
                                *state = ConnectState::Ready(Ok(stream));
                                // 转换为 Object
                                PollResult::Ready(tcp_stream_to_object(stream))
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // 仍在连接中，注册 waker
                                // TODO: 需要 I/O 多路复用
                                PollResult::Pending
                            }
                            Err(e) => {
                                PollResult::Rejected(error_to_object(e))
                            }
                        }
                    }
                    ConnectState::Ready(result) => {
                        // 已就绪
                        match result {
                            Ok(stream) => PollResult::Ready(tcp_stream_to_object(stream.try_clone().unwrap())),
                            Err(e) => PollResult::Rejected(error_to_object(e)),
                        }
                    }
                }
            }
            // ... 其他 I/O 操作
        }
    }
}
```

**问题**：非阻塞 I/O 需要操作系统级支持（epoll/kqueue），纯 Rust std 无法高效实现。

---

## 四、Tokio 集成方案（推荐）

### 4.1 为什么需要 Tokio？

| 功能 | 当前实现 | Tokio 方案 |
|------|---------|-----------|
| **定时器** | ✅ 时间轮 | ✅ 更高效的堆 |
| **网络 I/O** | ❌ 阻塞 | ✅ epoll/kqueue |
| **文件 I/O** | ❌ 阻塞 | ✅ 异步文件 |
| **并发任务** | ❌ 单线程 | ✅ 多线程池 |
| **性能** | 低 | 高 |

### 4.2 设计：Runtime trait 抽象

**目标**：支持多种运行时，透明切换

```rust
// src/object/runtime.rs
pub trait Runtime: 'static {
    /// 生成任务
    fn spawn(&self, awaitable: Box<dyn Awaitable>);
    
    /// 运行直到完成
    fn block_on(&self, awaitable: Box<dyn Awaitable>) -> PollResult;
    
    /// 异步睡眠
    fn sleep(&self, duration: Duration) -> Box<dyn Awaitable>;
    
    /// 异步 TCP 连接
    fn tcp_connect(&self, addr: &str) -> Box<dyn Awaitable>;
    
    /// 异步 TCP 读取
    fn tcp_read(&self, stream: TcpStreamHandle, buf: &mut [u8]) -> Box<dyn Awaitable>;
}

/// 当前的单线程运行时
pub struct NativeRuntime {
    event_loop: Rc<EventLoop>,
}

impl Runtime for NativeRuntime {
    fn block_on(&self, awaitable: Box<dyn Awaitable>) -> PollResult {
        self.event_loop.run(*awaitable)
    }
    
    fn sleep(&self, duration: Duration) -> Box<dyn Awaitable> {
        Box::new(SleepAwaitable::new(duration))
    }
    
    // ... 其他方法
}

/// Tokio 运行时（未来实现）
#[cfg(feature = "tokio-runtime")]
pub struct TokioRuntime {
    handle: tokio::runtime::Handle,
}

#[cfg(feature = "tokio-runtime")]
impl Runtime for TokioRuntime {
    fn block_on(&self, awaitable: Box<dyn Awaitable>) -> PollResult {
        self.handle.block_on(async {
            // 将 Awaitable 适配为 Tokio Future
            awaitable_to_future(awaitable).await
        })
    }
    
    fn sleep(&self, duration: Duration) -> Box<dyn Awaitable> {
        Box::new(TokioSleepAwaitable::new(duration))
    }
    
    fn tcp_connect(&self, addr: &str) -> Box<dyn Awaitable> {
        Box::new(TokioTcpConnectAwaitable::new(addr.to_string()))
    }
}
```

### 4.3 VirtualMachine 集成

```rust
// src/object/vm.rs
pub struct VirtualMachine {
    // ... 现有字段
    
    /// 异步运行时
    pub runtime: Box<dyn Runtime>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            runtime: Box::new(NativeRuntime::new()),
            // ...
        }
    }
    
    #[cfg(feature = "tokio-runtime")]
    pub fn with_tokio_runtime() -> Self {
        Self {
            runtime: Box::new(TokioRuntime::new()),
            // ...
        }
    }
}
```

### 4.4 Cargo.toml 配置

```toml
[features]
default = ["native-runtime"]
native-runtime = []
tokio-runtime = ["tokio"]

[dependencies]
tokio = { version = "1", features = ["full"], optional = true }
```

---

## 五、实施步骤

### 阶段 1：优化当前 EventLoop（1-2天）

1. ✅ 集成 TimerWheel 到 EventLoop
2. ✅ 优化空队列等待逻辑
3. ✅ 添加 next_deadline() 方法
4. ✅ 测试性能改进

**预期收益**：
- 减少 CPU 空转
- 定时器更精确

### 阶段 2：设计 Runtime trait（2-3天）

1. ✅ 定义 Runtime trait
2. ✅ 实现 NativeRuntime（基于现有 EventLoop）
3. ✅ 重构 VM 使用 Runtime
4. ✅ 测试兼容性

**预期收益**：
- 为 Tokio 集成做准备
- 代码更模块化

### 阶段 3：添加基础 I/O Awaitable（3-4天）

1. ✅ 实现 TcpConnectAwaitable
2. ✅ 实现 TcpReadAwaitable / TcpWriteAwaitable
3. ✅ 添加 asyncConnect/asyncRead 到 @std/net
4. ✅ 集成测试

**限制**：
- 仍基于轮询（poll + set_nonblocking）
- 性能不如 Tokio

### 阶段 4：Tokio 集成（1周）

1. ✅ 添加 tokio 依赖（feature gate）
2. ✅ 实现 TokioRuntime
3. ✅ Awaitable ↔ Future 适配器
4. ✅ 性能测试和优化

**预期收益**：
- 高性能异步 I/O
- 多线程支持
- 成熟的生态

---

## 六、API 示例

### 当前（同步阻塞）

```javascript
import { connect } from "@std/net/socket/client";

let conn = connect("localhost", 8080);  // 阻塞
conn.write("Hello");
let data = conn.read(1024);  // 阻塞
```

### 改进后（异步）

```javascript
import { asyncConnect } from "@std/net/socket/client";

async function main() {
    let conn = await asyncConnect("localhost", 8080);  // 非阻塞
    await conn.asyncWrite("Hello");
    let data = await conn.asyncRead(1024);  // 非阻塞
}
```

### Tokio 运行时（多线程）

```rust
// Rust 端启动脚本
let vm = VirtualMachine::with_tokio_runtime();
vm.run_script("main.gs");

// 脚本可以并发执行多个任务
async function server() {
    let listener = await asyncListen(8080);
    while (true) {
        let conn = await listener.accept();
        // 并发处理，不阻塞主线程
        spawn(handleClient(conn));
    }
}
```

---

## 七、风险和权衡

### 短期优化（阶段 1）
- ✅ **低风险**：只改进现有代码
- ✅ **低成本**：1-2 天
- ⚠️ **有限收益**：仍是单线程轮询

### Runtime trait（阶段 2）
- ✅ **架构改进**：为未来扩展做准备
- ⚠️ **需重构**：影响 VM 代码
- ✅ **向后兼容**：NativeRuntime 保持现有行为

### Tokio 集成（阶段 4）
- ✅ **巨大收益**：性能提升 10-100x
- ⚠️ **依赖膨胀**：tokio 是大依赖
- ⚠️ **复杂度**：Awaitable ↔ Future 适配
- ✅ **Feature gate**：可选功能，不强制

---

## 八、推荐方案

### 立即执行（本次会话）
**阶段 1：优化 EventLoop** ✅
- 集成 TimerWheel
- 优化空队列等待
- 低风险、快速见效

### 后续规划
**阶段 2：Runtime trait 设计** 📝
- 为长期架构打基础
- 作为独立任务

**阶段 3-4：I/O + Tokio** 🚀
- 作为"异步 I/O"特性开发
- 需要更多时间和测试

---

## 九、总结

### 当前限制
- ❌ 空队列时 sleep(1ms) 效率低
- ❌ 无异步 I/O 支持
- ❌ GTP 插件通信仍是同步的

### 改进路径
1. **短期**：优化 EventLoop（阶段 1）
2. **中期**：Runtime trait（阶段 2）
3. **长期**：Tokio 集成（阶段 4）

### 预期收益
- ✅ CPU 利用率优化
- ✅ 定时器更精确
- ✅ 为异步 I/O 做准备
- 🚀 未来支持高性能网络服务

---

**文档创建时间**: 2026-06-21  
**状态**: 设计方案  
**下一步**: 执行阶段 1 - 优化 EventLoop
