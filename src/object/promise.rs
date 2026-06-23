//! Promise values for the async model (single-threaded).
//!
//! A Promise resolves/rejects exactly once. This implementation now supports
//! both poll-based async (via Awaitable trait) and blocking wait (via wait()).

use std::cell::RefCell;
use std::rc::Rc;

use super::awaitable::{Awaitable, PollResult, Waker, WakerRegistry};
use super::value::Object;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseState {
    Pending,
    Fulfilled,
    Rejected,
}

struct PromiseInner {
    state: PromiseState,
    value: Option<Object>,
    wakers: WakerRegistry,
    continuations: Vec<PromiseContinuation>,
}

pub type PromiseContinuation = Box<dyn FnOnce(PromiseState, Object) + 'static>;

/// A Promise value.
pub struct Promise {
    inner: RefCell<PromiseInner>,
}

impl Promise {
    pub fn new() -> Rc<Promise> {
        Rc::new(Promise {
            inner: RefCell::new(PromiseInner {
                state: PromiseState::Pending,
                value: None,
                wakers: WakerRegistry::new(),
                continuations: Vec::new(),
            }),
        })
    }

    pub fn state(&self) -> PromiseState {
        self.inner.borrow().state
    }

    pub fn resolve(&self, value: Object) {
        self.settle(PromiseState::Fulfilled, value);
    }

    pub fn reject(&self, reason: Object) {
        self.settle(PromiseState::Rejected, reason);
    }

    fn settle(&self, state: PromiseState, value: Object) {
        let mut g = self.inner.borrow_mut();
        if g.state != PromiseState::Pending {
            return;
        }
        g.state = state;
        g.value = Some(value.clone());
        let wakers = std::mem::replace(&mut g.wakers, WakerRegistry::new());
        let continuations = std::mem::take(&mut g.continuations);
        drop(g);
        wakers.wake_all();
        for continuation in continuations {
            continuation(state, value.clone());
        }
    }

    pub fn add_continuation(&self, continuation: PromiseContinuation) {
        let mut g = self.inner.borrow_mut();
        if g.state == PromiseState::Pending {
            g.continuations.push(continuation);
            return;
        }
        let state = g.state;
        let value = g.value.clone().unwrap_or(Object::Undefined);
        drop(g);
        continuation(state, value);
    }

    /// Block until settled, returning the resolution value or rejection reason.
    pub fn wait(&self) -> Object {
        while self.state() == PromiseState::Pending {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.inner
            .borrow()
            .value
            .clone()
            .unwrap_or(Object::Undefined)
    }

    pub fn value(&self) -> Option<Object> {
        self.inner.borrow().value.clone()
    }

    pub fn inspect(&self) -> String {
        let g = self.inner.borrow();
        match g.state {
            PromiseState::Pending => "<promise pending>".to_string(),
            PromiseState::Fulfilled => match &g.value {
                Some(o) => format!("<promise resolved: {}>", o.inspect()),
                None => "<promise resolved>".to_string(),
            },
            PromiseState::Rejected => match &g.value {
                Some(o) => format!("<promise rejected: {}>", o.inspect()),
                None => "<promise rejected>".to_string(),
            },
        }
    }
}

impl Awaitable for Promise {
    /// Poll the Promise for readiness.
    ///
    /// If fulfilled, returns Ready(value).
    /// If rejected, returns Rejected(reason).
    /// If pending, registers the waker and returns Pending.
    fn poll(&self, waker: Waker) -> PollResult {
        let g = self.inner.borrow();
        match g.state {
            PromiseState::Fulfilled => {
                PollResult::Ready(g.value.clone().unwrap_or(Object::Undefined))
            }
            PromiseState::Rejected => {
                PollResult::Rejected(g.value.clone().unwrap_or(Object::Undefined))
            }
            PromiseState::Pending => {
                // Register waker for notification when promise settles
                g.wakers.register(waker);
                // Re-check after registering to handle race conditions
                if g.state != PromiseState::Pending {
                    // Settled between check and register - waker will be called
                    drop(g);
                    return self.poll(Rc::new(|| {})); // Re-poll with dummy waker
                }
                PollResult::Pending
            }
        }
    }
}
