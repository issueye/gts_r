# EventLoop Stage 3 - I/O Awaitable Support

## Overview

Stage 3 adds I/O Awaitable implementations for asynchronous network operations. This enables non-blocking TCP I/O that integrates with the native event loop.

## Implementation

### Files Created

1. **`src/object/io_awaitable.rs`** (285 lines)
   - `TcpConnectAwaitable` - Async TCP connection establishment
   - `TcpReadAwaitable` - Async reading from TCP streams
   - `TcpWriteAwaitable` - Async writing to TCP streams

2. **`src/async_runtime/mod.rs`** (updated)
   - Documentation of async runtime architecture
   - Design principles for tokio integration
   - Module structure for native and tokio runtimes

3. **`src/async_runtime/native.rs`** (updated)
   - Type alias for NativeRuntime = EventLoop
   - Helper function `create_runtime()`
   - Documentation of native runtime usage

4. **`src/async_runtime/TOKIO_INTEGRATION.md`** (318 lines)
   - Comprehensive tokio integration design document
   - Migration path and implementation strategy
   - Challenge analysis with recommended solutions

### Module Updates

- **`src/object/mod.rs`** - Added `io_awaitable` module and exports
- **`src/lib.rs`** - Added `async_runtime` module

## Architecture

### I/O Awaitable Pattern

Each I/O awaitable follows a consistent pattern:

```rust
pub struct TcpConnectAwaitable {
    addr: String,
    stream: RefCell<Option<Result<TcpStream, io::Error>>>,
    waker: RefCell<Option<Waker>>,
}

impl Awaitable for TcpConnectAwaitable {
    fn poll(&self, waker: Waker) -> PollResult {
        // 1. Try I/O operation on first poll
        // 2. Return Ready if successful
        // 3. Return Pending if WouldBlock (waker registered)
        // 4. Return Rejected on error
    }
}
```

### Design Decisions

1. **Non-blocking I/O**: All TCP operations use `set_nonblocking(true)`
2. **No thread spawning**: Wakers are stored but not invoked (event loop re-polls)
3. **Single-threaded**: Uses `Rc<RefCell<>>` for interior mutability
4. **Future-compatible**: Designed to integrate with epoll/kqueue/IOCP in Stage 4

### Current Limitations

1. **No I/O readiness system**: The event loop doesn't yet integrate with OS I/O multiplexing (epoll/kqueue/IOCP)
2. **Busy polling**: Pending awaitables are re-polled by the event loop without waiting for I/O readiness
3. **No waker invocation**: Wakers are stored but not called (would require I/O readiness notification)
4. **Blocking under the hood**: While the API is async, actual I/O may block briefly

These limitations are acceptable for Stage 3, as they will be addressed when adding proper I/O multiplexing in Stage 4.

## Usage Example

```rust
use gts::object::{TcpConnectAwaitable, EventLoop};

// Create event loop
let event_loop = EventLoop::new();

// Create TCP connect awaitable
let connect = TcpConnectAwaitable::new("127.0.0.1:8080");

// Spawn the awaitable
event_loop.spawn(connect, |result| {
    match result {
        PollResult::Ready(fd) => println!("Connected: {:?}", fd),
        PollResult::Rejected(err) => println!("Error: {:?}", err),
        _ => {}
    }
});

// Run event loop
event_loop.run_until_quiescent();
```

## Testing

- **Compilation**: ✅ 0 errors, 32 warnings
- **Unit tests**: ✅ 19 tests passed (including 1 new native runtime test)
- **Integration**: Pending (requires stdlib integration)

## Next Steps (Stage 4)

The next stage will add proper I/O readiness tracking:

1. **I/O Multiplexing**:
   - Implement epoll/kqueue/IOCP abstraction
   - Register file descriptors with readiness system
   - Invoke wakers when I/O becomes ready

2. **EventLoop Integration**:
   - Add I/O readiness polling to `wait_for_events()`
   - Balance between timer deadlines and I/O readiness
   - Support mixed timer and I/O workloads

3. **Platform Support**:
   - Linux: epoll
   - macOS/BSD: kqueue
   - Windows: IOCP
   - Fallback: polling with short timeouts

4. **Tokio Bridge** (optional, feature-gated):
   - Convert I/O awaitables to tokio futures
   - Use tokio's multi-threaded I/O system
   - Enable true parallelism for I/O-bound workloads

## Files Modified

- `src/lib.rs` - Added `async_runtime` module (1 line)
- `src/object/mod.rs` - Added `io_awaitable` module and exports (2 lines)
- `src/object/io_awaitable.rs` - Created (285 lines)
- `src/async_runtime/mod.rs` - Rewritten with architecture docs (48 lines)
- `src/async_runtime/native.rs` - Simplified to type alias (83 lines)
- `src/async_runtime/TOKIO_INTEGRATION.md` - Created (318 lines)

## Compilation Stats

- Build time: 4.42s
- Warnings: 32 (mostly unused imports, unrelated to this stage)
- Binary size: Not measured
- Test time: 0.06s

## Conclusion

Stage 3 successfully adds I/O Awaitable support with a clean, composable API. The implementation is single-threaded and doesn't yet integrate with OS I/O multiplexing, but provides the foundation for both native I/O readiness (Stage 4) and tokio integration (future work).

The key achievement is establishing the Awaitable pattern for I/O operations, which can be seamlessly upgraded to use epoll/kqueue/IOCP without changing the public API.
