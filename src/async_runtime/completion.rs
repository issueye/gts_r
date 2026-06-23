//! Thread-safe async completion queue for handing owned results back to the VM.
//!
//! VM objects are deliberately absent from this module. Tokio/background work
//! can clone an `AsyncCompletionSender`, send owned data, and leave all object
//! resolution for the VM thread that drains the queue.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A logical async operation identifier allocated on the VM thread.
pub type AsyncCompletionId = u64;

/// Owned data that may cross from background workers back to the VM thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncCompletionData {
    Undefined,
    Text(String),
    Bytes(Vec<u8>),
    HttpResponse(AsyncHttpResponse),
    JsonText(String),
}

/// An owned HTTP response payload suitable for cross-thread delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncHttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The result of a background async operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncCompletionResult {
    Resolve(AsyncCompletionData),
    Reject(String),
}

/// A queued completion to be drained by the VM thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncCompletion {
    pub id: AsyncCompletionId,
    pub result: AsyncCompletionResult,
}

impl AsyncCompletion {
    pub fn resolve(id: AsyncCompletionId, data: AsyncCompletionData) -> Self {
        Self {
            id,
            result: AsyncCompletionResult::Resolve(data),
        }
    }

    pub fn reject(id: AsyncCompletionId, error: impl Into<String>) -> Self {
        Self {
            id,
            result: AsyncCompletionResult::Reject(error.into()),
        }
    }
}

#[derive(Debug, Default)]
struct AsyncCompletionQueueInner {
    pending: VecDeque<AsyncCompletion>,
}

/// Cloneable sender safe to move into Tokio/background threads.
#[derive(Debug, Clone, Default)]
pub struct AsyncCompletionSender {
    inner: Arc<Mutex<AsyncCompletionQueueInner>>,
}

impl AsyncCompletionSender {
    pub fn enqueue(&self, completion: AsyncCompletion) {
        self.inner
            .lock()
            .expect("async completion queue poisoned")
            .pending
            .push_back(completion);
    }

    pub fn resolve(&self, id: AsyncCompletionId, data: AsyncCompletionData) {
        self.enqueue(AsyncCompletion::resolve(id, data));
    }

    pub fn reject(&self, id: AsyncCompletionId, error: impl Into<String>) {
        self.enqueue(AsyncCompletion::reject(id, error));
    }
}

/// VM-side handle for draining queued async completions.
#[derive(Debug, Clone, Default)]
pub struct AsyncCompletionQueue {
    sender: AsyncCompletionSender,
}

impl AsyncCompletionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sender(&self) -> AsyncCompletionSender {
        self.sender.clone()
    }

    pub fn enqueue(&self, completion: AsyncCompletion) {
        self.sender.enqueue(completion);
    }

    pub fn drain(&self) -> Vec<AsyncCompletion> {
        let mut inner = self
            .sender
            .inner
            .lock()
            .expect("async completion queue poisoned");
        inner.pending.drain(..).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.sender
            .inner
            .lock()
            .expect("async completion queue poisoned")
            .pending
            .is_empty()
    }

    pub fn len(&self) -> usize {
        self.sender
            .inner
            .lock()
            .expect("async completion queue poisoned")
            .pending
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_in_fifo_order() {
        let queue = AsyncCompletionQueue::new();
        let sender = queue.sender();

        sender.resolve(1, AsyncCompletionData::Text("one".into()));
        sender.reject(2, "two failed");

        let completions = queue.drain();
        assert_eq!(completions.len(), 2);
        assert_eq!(
            completions[0],
            AsyncCompletion::resolve(1, AsyncCompletionData::Text("one".into()))
        );
        assert_eq!(completions[1], AsyncCompletion::reject(2, "two failed"));
        assert!(queue.is_empty());
    }
}
