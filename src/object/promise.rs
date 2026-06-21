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
}

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
            }),
        })
    }

    pub fn state(&self) -> PromiseState {
        self.inner.borrow().state
    }

    pub fn resolve(&self, value: Object) {
        let mut g = self.inner.borrow_mut();
        if g.state != PromiseState::Pending {
            return;
        }
        g.state = PromiseState::Fulfilled;
        g.value = Some(value);
        // Wake all tasks waiting on this promise
        let wakers = std::mem::replace(&mut g.wakers, WakerRegistry::new());
        drop(g); // Release borrow before calling wakers
        wakers.wake_all();
    }

    pub fn reject(&self, reason: Object) {
        let mut g = self.inner.borrow_mut();
        if g.state != PromiseState::Pending {
            return;
        }
        g.state = PromiseState::Rejected;
        g.value = Some(reason);
        // Wake all tasks waiting on this promise
        let wakers = std::mem::replace(&mut g.wakers, WakerRegistry::new());
        drop(g); // Release borrow before calling wakers
        wakers.wake_all();
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
