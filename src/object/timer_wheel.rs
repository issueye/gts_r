//! TimerWheel — simplified timer scheduler for single-threaded async runtime.
//!
//! This is a simplified version of the Go implementation. Instead of a separate
//! driver thread, timers are checked when the event loop polls. This works well
//! for a single-threaded interpreter.

use std::cell::RefCell;
use std::collections::BinaryHeap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::awaitable::{Awaitable, PollResult, Waker};

/// A timer entry in the heap.
#[derive(Clone)]
struct TimerEntry {
    deadline: Instant,
    waker: Option<Waker>,
    id: u64,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order for min-heap (BinaryHeap is max-heap)
        other.deadline.cmp(&self.deadline)
    }
}

/// TimerWheel manages scheduled timers.
pub struct TimerWheel {
    heap: RefCell<BinaryHeap<TimerEntry>>,
    next_id: RefCell<u64>,
}

impl TimerWheel {
    pub fn new() -> Rc<Self> {
        Rc::new(TimerWheel {
            heap: RefCell::new(BinaryHeap::new()),
            next_id: RefCell::new(0),
        })
    }

    /// Create a new TimerWheel without wrapping in Rc.
    /// Used when the caller will wrap it themselves.
    pub fn new_unwrapped() -> Self {
        TimerWheel {
            heap: RefCell::new(BinaryHeap::new()),
            next_id: RefCell::new(0),
        }
    }

    /// Schedule a timer that fires after the given delay.
    pub fn schedule(&self, delay: Duration, waker: Waker) -> u64 {
        let id = {
            let mut next_id = self.next_id.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let entry = TimerEntry {
            deadline: Instant::now() + delay,
            waker: Some(waker),
            id,
        };

        self.heap.borrow_mut().push(entry);
        id
    }

    /// Cancel a scheduled timer by id.
    pub fn cancel(&self, id: u64) {
        let mut heap = self.heap.borrow_mut();
        // Remove by marking waker as None (lazy deletion)
        let entries: Vec<_> = heap.drain().collect();
        for mut entry in entries {
            if entry.id == id {
                entry.waker = None;
            }
            if entry.waker.is_some() {
                heap.push(entry);
            }
        }
    }

    /// Fire all expired timers.
    pub fn fire_expired(&self) {
        let now = Instant::now();
        let mut heap = self.heap.borrow_mut();

        while let Some(entry) = heap.peek() {
            if entry.deadline > now {
                break; // Not yet expired
            }

            let entry = heap.pop().unwrap();
            if let Some(waker) = entry.waker {
                drop(heap); // Release borrow before calling waker
                waker();
                heap = self.heap.borrow_mut();
            }
        }
    }

    /// Get the duration until the next timer fires, or None if no timers.
    pub fn time_until_next(&self) -> Option<Duration> {
        let heap = self.heap.borrow();
        heap.peek().map(|entry| {
            let now = Instant::now();
            if entry.deadline > now {
                entry.deadline - now
            } else {
                Duration::from_millis(0)
            }
        })
    }

    /// Get the absolute deadline of the next timer, or None if no timers.
    pub fn next_deadline(&self) -> Option<Instant> {
        let heap = self.heap.borrow();
        heap.peek().map(|entry| entry.deadline)
    }

    /// Tick the timer wheel - fire all expired timers.
    /// This is an alias for fire_expired() for consistency with EventLoop.
    pub fn tick(&self) {
        self.fire_expired();
    }
}

impl Default for TimerWheel {
    fn default() -> Self {
        TimerWheel {
            heap: RefCell::new(BinaryHeap::new()),
            next_id: RefCell::new(0),
        }
    }
}

/// TimerAwaitable represents a timer that can be awaited.
pub struct TimerAwaitable {
    deadline: Instant,
    fired: RefCell<bool>,
}

impl TimerAwaitable {
    pub fn new(delay: Duration) -> Self {
        TimerAwaitable {
            deadline: Instant::now() + delay,
            fired: RefCell::new(false),
        }
    }
}

impl Awaitable for TimerAwaitable {
    fn poll(&self, _waker: Waker) -> PollResult {
        if *self.fired.borrow() {
            return PollResult::Ready(crate::object::Object::Undefined);
        }

        let now = Instant::now();
        if now >= self.deadline {
            *self.fired.borrow_mut() = true;
            PollResult::Ready(crate::object::Object::Undefined)
        } else {
            // Timer not yet expired - in a single-threaded model, the event loop
            // will poll again after processing other tasks
            // TODO: Integrate with event loop's timer wheel for efficient scheduling
            PollResult::Pending
        }
    }
}
