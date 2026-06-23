# Tokio Integration Design

This document outlines the strategy for integrating tokio into the GTS runtime while maintaining backward compatibility with the existing single-threaded EventLoop.

## Goals

1. **Optional parallelism**: Enable multi-threaded execution via tokio when the `tokio` feature is enabled
2. **Zero overhead**: No tokio dependency or runtime cost when feature is disabled
3. **Backward compatibility**: Existing code works unchanged with native runtime
4. **Gradual migration**: Components can adopt tokio incrementally

## Architecture Overview

### Current State (Native Runtime)

```
┌─────────────────┐
│  VirtualMachine │
└────────┬────────┘
         │
    ┌────▼─────────┐
    │  EventLoop   │──► Single-threaded poll loop
    └────┬─────────┘
         │
    ┌────▼─────────┐
    │  TimerWheel  │──► Binary heap timers
    └──────────────┘
         │
    ┌────▼─────────┐
    │  Awaitable   │──► Rc<dyn Awaitable>
    └──────────────┘
```

### Target State (Tokio Runtime)

```
┌─────────────────┐
│  VirtualMachine │
└────────┬────────┘
         │
    ┌────▼─────────────┐
    │  RuntimeBridge   │──► Dispatch to native or tokio
    └────┬─────────────┘
         │
    ┌────▼──────────────────────────────┐
    │  Tokio Runtime (multi-threaded)   │
    └────┬──────────────────────────────┘
         │
    ┌────▼─────────────────┐
    │  AwaitableBridge     │──► Convert Rc → Arc
    └──────────────────────┘
```

## Implementation Strategy

### Phase 1: Thread-Safety Abstraction

Add a feature-gated smart pointer abstraction:

```rust
#[cfg(feature = "tokio")]
pub type SharedPtr<T> = Arc<T>;
#[cfg(feature = "tokio")]
pub type SharedCell<T> = Mutex<T>;

#[cfg(not(feature = "tokio"))]
pub type SharedPtr<T> = Rc<T>;
#[cfg(not(feature = "tokio"))]
pub type SharedCell<T> = RefCell<T>;
```

**Impact**: This requires changing `Rc<T>` → `SharedPtr<T>` throughout the codebase.

**Alternative**: Keep Rc/RefCell for native, use separate Arc/Mutex types for tokio bridge.

### Phase 2: Runtime Trait Abstraction

Define a common interface for async operations:

```rust
pub trait AsyncRuntime {
    type Handle;
    
    fn spawn_awaitable(&self, awaitable: Box<dyn Awaitable + Send>) -> Self::Handle;
    fn set_timer(&self, duration: Duration, callback: Box<dyn FnOnce() + Send>) -> TimerId;
    fn cancel_timer(&self, id: TimerId);
    fn block_on<F: Future>(&self, future: F) -> F::Output;
}
```

**Issues**:
- Current `Awaitable` trait is not `Send` (uses `Rc`)
- Converting between native and tokio contexts requires synchronization

### Phase 3: Bridge Implementation

Create adapters between GTS and tokio primitives:

```rust
#[cfg(feature = "tokio")]
struct TokioAwaitableBridge {
    awaitable: Arc<Mutex<Box<dyn Awaitable + Send>>>,
}

impl Future for TokioAwaitableBridge {
    type Output = PollResult;
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let waker = cx.waker().clone();
        let gts_waker = Arc::new(move || waker.wake_by_ref());
        
        match self.awaitable.lock().unwrap().poll(gts_waker) {
            PollResult::Ready(val) => Poll::Ready(PollResult::Ready(val)),
            PollResult::Rejected(err) => Poll::Ready(PollResult::Rejected(err)),
            PollResult::Pending => Poll::Pending,
        }
    }
}
```

### Phase 4: VM Integration

Add runtime selection to VirtualMachine:

```rust
pub struct VirtualMachine {
    // ... existing fields ...
    
    #[cfg(feature = "tokio")]
    tokio_runtime: Option<tokio::runtime::Runtime>,
    
    #[cfg(not(feature = "tokio"))]
    event_loop: Rc<EventLoop>,
}

impl VirtualMachine {
    pub fn new() -> Rc<Self> {
        // Create VM with tokio runtime
    }
    
    pub fn with_native() -> Rc<Self> {
        // Create VM with native event loop (default)
    }
}
```

## Challenges and Solutions

### Challenge 1: Rc → Arc Conversion

**Problem**: Existing code uses `Rc<T>` which is not `Send`.

**Solutions**:
- A. Convert entire codebase to use `Arc<T>` (breaking change)
- B. Use conditional compilation to switch types (complex)
- C. Keep separate native and tokio code paths (duplication)

**Recommended**: Option C - maintain separate implementations with shared interfaces.

### Challenge 2: RefCell → Mutex Performance

**Problem**: Replacing `RefCell` with `Mutex` adds synchronization overhead.

**Solutions**:
- A. Accept the overhead when tokio is enabled
- B. Use `parking_lot::Mutex` for better performance
- C. Keep native path using RefCell, only tokio path uses Mutex

**Recommended**: Option C - native path remains zero-cost.

### Challenge 3: Awaitable Trait Not Send

**Problem**: Current `Awaitable` trait cannot cross thread boundaries.

**Solutions**:
- A. Make `Awaitable: Send` (requires Arc/Mutex everywhere)
- B. Create separate `SendAwaitable` trait for tokio
- C. Box and wrap in Arc<Mutex<>> when crossing to tokio

**Recommended**: Option B - introduce `SendAwaitable` for tokio path.

### Challenge 4: EventLoop Single-threaded Assumption

**Problem**: EventLoop assumes single-threaded execution (no locks).

**Solutions**:
- A. Make EventLoop thread-safe (adds overhead)
- B. Run EventLoop on dedicated thread, communicate via channels
- C. Replace EventLoop entirely in tokio mode

**Recommended**: Option C - tokio has its own scheduler.

## Migration Path

### Step 1: Add tokio Feature Flag

```toml
[features]
default = []
tokio = ["dep:tokio"]

[dependencies]
tokio = { version = "1", features = ["full"], optional = true }
```

### Step 2: Implement TokioRuntime Module

Create `src/async_runtime/tokio_rt.rs` with:
- Tokio runtime wrapper
- Awaitable → Future bridge
- Timer adaptation
- I/O integration

### Step 3: Add Runtime Selection API

```rust
// Default: native runtime
let session = Session::new();

// Opt-in: tokio runtime
#[cfg(feature = "tokio")]
let session = Session::new();
```

### Step 4: Feature-gate stdlib I/O

```rust
#[cfg(feature = "tokio")]
async fn tcp_connect(addr: &str) -> Result<TcpStream, Error> {
    tokio::net::TcpStream::connect(addr).await
}

#[cfg(not(feature = "tokio"))]
async fn tcp_connect(addr: &str) -> Result<TcpStream, Error> {
    // Native blocking I/O wrapped in Awaitable
}
```

## Testing Strategy

1. **Dual compilation**: CI runs tests with both `--no-default-features` and `--features tokio`
2. **Runtime parity tests**: Same test suite runs on both runtimes
3. **Performance benchmarks**: Compare native vs tokio overhead
4. **Integration tests**: Test tokio-specific features (parallel execution)

## Performance Considerations

### Native Runtime (default)

- **Pros**: Zero overhead, simple debugging, no thread synchronization
- **Cons**: Single-threaded, blocking I/O blocks event loop

### Tokio Runtime (opt-in)

- **Pros**: Multi-threaded, non-blocking I/O, better scalability
- **Cons**: Thread synchronization overhead, complex debugging, larger binary

## Timeline

- **Stage 1** (Current): Document architecture, design abstractions ✅
- **Stage 2** (Next): Add I/O Awaitable support to native runtime
- **Stage 3**: Implement tokio feature flag and basic bridge
- **Stage 4**: Feature-gate stdlib I/O for tokio
- **Stage 5**: Performance tuning and optimization
- **Stage 6**: Documentation and examples

## Conclusion

The recommended approach is to maintain separate native and tokio code paths, with:
- Feature flag controlling which runtime is compiled
- Shared high-level APIs (Session, VirtualMachine)
- Runtime-specific implementations hidden behind modules
- Zero overhead for native path, opt-in complexity for tokio

This design preserves the simplicity of the single-threaded native runtime while enabling multi-threaded parallelism when needed.
