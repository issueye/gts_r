/// Awaitable Bridge - Connects GTS Awaitables to Tokio Futures
///
/// This module provides a bridge between GTS's single-threaded Awaitable
/// system and tokio's multi-threaded Future-based async runtime.
///
/// ## The Challenge
///
/// GTS Awaitables use `Rc<dyn Fn()>` for wakers, which is not thread-safe.
/// Tokio Futures require `Send + 'static`, which `Rc` doesn't satisfy.
///
/// ## Solution
///
/// Rather than converting Awaitables directly to Futures (which is
/// fundamentally incompatible), we provide:
///
/// 1. **Helper functions** to run async operations on tokio
/// 2. **Message passing** to coordinate between runtimes
/// 3. **Serialization** of results across thread boundaries
///
/// ## Usage Pattern
///
/// Instead of converting Awaitable → Future, use tokio directly:
///
/// ```rust,ignore
/// // DON'T: Try to convert Awaitable to Future
/// // let future = AwaitableFuture::new(awaitable); // Won't work!
///
/// // DO: Use tokio's async primitives directly
/// use gts::async_runtime::tokio_rt::tcp;
/// let stream = tcp::connect("127.0.0.1:8080").await?;
/// ```

#[cfg(feature = "tokio")]
use std::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::sync::Mutex;

#[cfg(feature = "tokio")]
use crate::object::PollResult;

/// Serialize a PollResult for thread-safe transfer
///
/// Since PollResult contains GTS Objects (which use Rc/RefCell),
/// we serialize them to strings for transfer across threads.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone)]
pub enum SerializedResult {
    Ready(String),
    Rejected(String),
    Pending,
}

#[cfg(feature = "tokio")]
impl SerializedResult {
    /// Serialize a PollResult
    pub fn from_poll_result(result: &PollResult) -> Self {
        match result {
            PollResult::Ready(obj) => SerializedResult::Ready(obj.inspect()),
            PollResult::Rejected(err) => SerializedResult::Rejected(err.inspect()),
            PollResult::Pending => SerializedResult::Pending,
        }
    }

    /// Check if this is Ready
    pub fn is_ready(&self) -> bool {
        matches!(self, SerializedResult::Ready(_))
    }

    /// Check if this is Rejected
    pub fn is_rejected(&self) -> bool {
        matches!(self, SerializedResult::Rejected(_))
    }

    /// Get the value if Ready
    pub fn as_ready(&self) -> Option<&str> {
        match self {
            SerializedResult::Ready(s) => Some(s),
            _ => None,
        }
    }

    /// Get the error if Rejected
    pub fn as_rejected(&self) -> Option<&str> {
        match self {
            SerializedResult::Rejected(s) => Some(s),
            _ => None,
        }
    }
}

/// Shared state for coordinating async operations
///
/// This allows passing async work between the main GTS thread
/// and tokio worker threads.
#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct AsyncCoordinator {
    /// Pending async operations
    pending: Arc<Mutex<Vec<String>>>,
}

#[cfg(feature = "tokio")]
impl AsyncCoordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        AsyncCoordinator {
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a pending operation
    pub async fn add_pending(&self, operation: String) {
        let mut pending = self.pending.lock().await;
        pending.push(operation);
    }

    /// Get pending operations
    pub async fn get_pending(&self) -> Vec<String> {
        let mut pending = self.pending.lock().await;
        std::mem::take(&mut *pending)
    }
}

#[cfg(feature = "tokio")]
impl Default for AsyncCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to spawn a blocking GTS operation on tokio
///
/// This runs the operation on tokio's blocking thread pool,
/// which is appropriate for CPU-bound work.
#[cfg(feature = "tokio")]
pub async fn spawn_blocking_gts<F, R>(f: F) -> Result<R, tokio::task::JoinError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f).await
}

/// Documentation and usage examples
pub mod docs {
    /// Example: Using tokio with GTS Session
    ///
    /// ```rust,ignore
    /// use gts::runtime::Session;
    /// use gts::async_runtime::tokio_rt::TokioRuntime;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let session = Session::new();
    ///     
    ///     // Run GTS script on blocking thread pool
    ///     let result = tokio::task::spawn_blocking(move || {
    ///         session.run_source("console.log('Hello!')", "script.gs")
    ///     }).await.unwrap();
    ///     
    ///     println!("Result: {:?}", result);
    /// }
    /// ```
    pub fn example_session() {}

    /// Example: Mixing tokio I/O with GTS
    ///
    /// ```rust,ignore
    /// use gts::async_runtime::tokio_rt::tcp;
    /// use gts::runtime::Session;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Use tokio for async I/O
    ///     let mut stream = tcp::connect("127.0.0.1:8080".to_string()).await?;
    ///     let data = b"GET / HTTP/1.1\r\n\r\n";
    ///     tcp::write(&mut stream, data).await?;
    ///     
    ///     let mut buf = vec![0u8; 4096];
    ///     let n = tcp::read(&mut stream, &mut buf).await?;
    ///     
    ///     // Process response with GTS on blocking thread
    ///     let session = Session::new();
    ///     let response = String::from_utf8_lossy(&buf[..n]);
    ///     
    ///     tokio::task::spawn_blocking(move || {
    ///         session.run_source(
    ///             &format!("const response = '{}'; /* process */", response),
    ///             "process.gs"
    ///         )
    ///     }).await.unwrap();
    /// }
    /// ```
    pub fn example_mixed_io() {}

    /// Architecture Note: Why No Direct Conversion?
    ///
    /// Converting Awaitable → Future is fundamentally problematic:
    ///
    /// 1. **Waker incompatibility**:
    ///    - Awaitable uses `Rc<dyn Fn()>` (not Send)
    ///    - Future uses `Arc<Waker>` (Send + Sync)
    ///
    /// 2. **Object model**:
    ///    - GTS Objects use `Rc<RefCell<T>>` (not Send)
    ///    - Tokio requires Send bounds
    ///
    /// 3. **Thread safety**:
    ///    - GTS is single-threaded by design
    ///    - Tokio is multi-threaded
    ///
    /// ## Recommended Patterns
    ///
    /// 1. **Keep GTS single-threaded**: Run GTS code on one thread,
    ///    use tokio only for I/O
    ///
    /// 2. **Message passing**: Use channels to coordinate
    ///
    /// 3. **Serialize results**: Convert Objects to strings/JSON
    ///    for thread boundaries
    ///
    /// 4. **spawn_blocking**: Use tokio's blocking pool for
    ///    CPU-bound GTS work
    pub fn architecture_notes() {}
}

#[cfg(all(test, feature = "tokio"))]
mod tests {
    use super::*;

    #[test]
    fn test_serialized_result() {
        let ready = SerializedResult::Ready("42".to_string());
        assert!(ready.is_ready());
        assert_eq!(ready.as_ready(), Some("42"));

        let rejected = SerializedResult::Rejected("error".to_string());
        assert!(rejected.is_rejected());
        assert_eq!(rejected.as_rejected(), Some("error"));

        let pending = SerializedResult::Pending;
        assert!(!pending.is_ready());
        assert!(!pending.is_rejected());
    }

    #[tokio::test]
    async fn test_async_coordinator() {
        let coord = AsyncCoordinator::new();

        coord.add_pending("op1".to_string()).await;
        coord.add_pending("op2".to_string()).await;

        let pending = coord.get_pending().await;
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0], "op1");
        assert_eq!(pending[1], "op2");

        // Should be cleared after get_pending
        let empty = coord.get_pending().await;
        assert_eq!(empty.len(), 0);
    }

    #[tokio::test]
    async fn test_spawn_blocking_gts() {
        let result = spawn_blocking_gts(|| {
            // Simulate some GTS work
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        })
        .await
        .unwrap();

        assert_eq!(result, 42);
    }
}
