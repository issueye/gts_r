# Tokio Integration Example

This document demonstrates how to use the tokio runtime with GTS.

## Building with Tokio Support

```bash
# Build with tokio feature
cargo build --features tokio

# Run tests with tokio
cargo test --features tokio

# Build binary with tokio
cargo build --bin gs --features tokio
```

## Basic Usage

### Creating a Tokio-Enabled Session

```rust
use gts::runtime::Session;

#[cfg(feature = "tokio")]
fn main() {
    // Create session with tokio runtime
    let session = Session::with_tokio();
    
    // Check if tokio is available
    assert!(session.has_tokio());
    
    // Run a script
    let result = session.run_source(
        r#"
        console.log("Hello from GTS with Tokio!");
        "#,
        "example.gs"
    );
    
    println!("Result: {:?}", result);
}

#[cfg(not(feature = "tokio"))]
fn main() {
    // Fallback to native runtime
    let session = Session::new();
    
    let result = session.run_source(
        r#"
        console.log("Hello from GTS!");
        "#,
        "example.gs"
    );
    
    println!("Result: {:?}", result);
}
```

## Async I/O with Tokio

When the tokio feature is enabled, you can use tokio's async I/O primitives directly:

```rust
#[cfg(feature = "tokio")]
use gts::async_runtime::tokio_rt::{TokioRuntime, tcp};

#[cfg(feature = "tokio")]
async fn async_tcp_example() {
    // Connect to a TCP server
    let mut stream = tcp::connect("127.0.0.1:8080".to_string())
        .await
        .expect("Failed to connect");
    
    // Write data
    let data = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    tcp::write(&mut stream, data)
        .await
        .expect("Failed to write");
    
    // Read response
    let mut buffer = vec![0u8; 4096];
    let n = tcp::read(&mut stream, &mut buffer)
        .await
        .expect("Failed to read");
    
    println!("Received {} bytes", n);
}

#[cfg(feature = "tokio")]
fn main() {
    let runtime = TokioRuntime::new();
    runtime.block_on(async_tcp_example());
}
```

## Multi-threaded Task Execution

```rust
#[cfg(feature = "tokio")]
use gts::async_runtime::tokio_rt::TokioRuntime;

#[cfg(feature = "tokio")]
fn main() {
    let runtime = TokioRuntime::with_worker_threads(8);
    
    // Spawn multiple tasks that run concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            runtime.spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                println!("Task {} completed", i);
                i * 2
            })
        })
        .collect();
    
    // Wait for all tasks
    runtime.block_on(async {
        for handle in handles {
            let result = handle.await.unwrap();
            println!("Result: {}", result);
        }
    });
}
```

## Performance Comparison

### Native Runtime (Default)

**Pros:**
- Zero tokio overhead
- Simple single-threaded execution
- Easy debugging
- Faster for CPU-bound scripts

**Cons:**
- Blocking I/O blocks entire event loop
- Single-threaded (no parallelism)
- Limited scalability for I/O-bound workloads

### Tokio Runtime (Feature-Gated)

**Pros:**
- Multi-threaded execution
- Non-blocking I/O with epoll/kqueue/IOCP
- Better scalability for concurrent connections
- Faster for I/O-bound scripts

**Cons:**
- Thread synchronization overhead
- Larger binary size (~2MB for tokio)
- More complex debugging
- Requires thread-safe data structures for shared state

## Benchmark Example

```rust
#[cfg(feature = "tokio")]
use std::time::Instant;
use gts::runtime::Session;

fn benchmark_concurrent_io() {
    let start = Instant::now();
    
    #[cfg(feature = "tokio")]
    {
        let session = Session::with_tokio();
        let runtime = session.tokio_runtime().unwrap();
        
        // Simulate 100 concurrent I/O operations
        let handles: Vec<_> = (0..100)
            .map(|_| {
                runtime.spawn(async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                })
            })
            .collect();
        
        runtime.block_on(async {
            for handle in handles {
                handle.await.unwrap();
            }
        });
    }
    
    #[cfg(not(feature = "tokio"))]
    {
        // Native runtime - would take 100 * 10ms = 1 second
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    
    let elapsed = start.elapsed();
    println!("Completed in {:?}", elapsed);
    
    #[cfg(feature = "tokio")]
    println!("With tokio: ~10ms (parallel execution)");
    
    #[cfg(not(feature = "tokio"))]
    println!("Without tokio: ~1000ms (sequential execution)");
}
```

## Feature Detection in Scripts

GTS scripts can detect if tokio is available:

```javascript
// example.gs
if (typeof process.tokio !== 'undefined') {
    console.log("Tokio runtime is available!");
    // Use async I/O operations
} else {
    console.log("Native runtime only");
    // Use synchronous I/O operations
}
```

## Binary Size Impact

- **Without tokio**: ~15 MB (release build)
- **With tokio**: ~17 MB (release build)
- **Size increase**: ~2 MB (~13%)

The tokio dependency adds approximately 2MB to the binary size, which is acceptable for most use cases where the performance benefits outweigh the size cost.

## When to Use Tokio

**Use tokio when:**
- Your scripts perform heavy I/O (network, file system)
- You need to handle many concurrent connections
- You want parallel task execution
- Performance is critical for I/O-bound workloads

**Use native runtime when:**
- Your scripts are CPU-bound
- You prefer simpler single-threaded execution
- Binary size is a constraint
- You don't need concurrent I/O

## Conclusion

The tokio integration provides optional multi-threaded async execution for GTS scripts. It's fully feature-gated, so you can choose at compile time whether to include tokio support. The API is designed to be ergonomic and safe, bridging GTS's single-threaded object model with tokio's multi-threaded runtime.
