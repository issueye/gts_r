//! EventLoop — GTS's analog of the tokio executor.
//!
//! A single-threaded event loop that drives Awaitables via polling. It maintains
//! a ready queue of tasks; each task wraps an Awaitable and is re-polled when
//! its waker fires.
//!
//! This is the runtime that makes poll-based async actually work. Tasks are
//! created from Awaitables (Promises, timers, select futures), polled for
//! readiness, and parked until woken.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::awaitable::{Awaitable, PollResult, Waker};
use super::timer_wheel::TimerWheel;

#[cfg(not(feature = "tokio"))]
use super::io_selector::{Event, Interest, IoSelector, RawFd, Selector, Token};

/// A scheduled task: an Awaitable being polled to completion.
struct Task {
    awaitable: Rc<dyn Awaitable>,
    on_done: Option<Box<dyn FnOnce(PollResult)>>,
}

/// I/O registration for tracking file descriptor interests
#[cfg(not(feature = "tokio"))]
struct IoRegistration {
    waker: Waker,
    interest: Interest,
}

/// EventLoop drives Awaitables on a single thread.
pub struct EventLoop {
    ready_queue: RefCell<VecDeque<Rc<RefCell<Task>>>>,
    timer_wheel: Rc<RefCell<TimerWheel>>,
    #[cfg(not(feature = "tokio"))]
    io_selector: RefCell<Selector>,
    #[cfg(not(feature = "tokio"))]
    io_registrations: RefCell<HashMap<Token, IoRegistration>>,
    #[cfg(not(feature = "tokio"))]
    next_token: RefCell<usize>,
}

impl EventLoop {
    pub fn new() -> Rc<Self> {
        Rc::new(EventLoop {
            ready_queue: RefCell::new(VecDeque::new()),
            timer_wheel: Rc::new(RefCell::new(TimerWheel::new_unwrapped())),
            #[cfg(not(feature = "tokio"))]
            io_selector: RefCell::new(
                super::io_selector::new_selector().expect("Failed to create I/O selector"),
            ),
            #[cfg(not(feature = "tokio"))]
            io_registrations: RefCell::new(HashMap::new()),
            #[cfg(not(feature = "tokio"))]
            next_token: RefCell::new(1),
        })
    }

    /// Get a reference to the timer wheel
    pub fn timer_wheel(&self) -> &Rc<RefCell<TimerWheel>> {
        &self.timer_wheel
    }

    /// Register a file descriptor for I/O events
    ///
    /// Returns a token that will be provided when the I/O is ready.
    #[cfg(not(feature = "tokio"))]
    pub fn register_io(
        &self,
        fd: RawFd,
        interest: Interest,
        waker: Waker,
    ) -> std::io::Result<Token> {
        let token = Token(*self.next_token.borrow());
        *self.next_token.borrow_mut() += 1;

        self.io_selector
            .borrow_mut()
            .register(fd, token, interest)?;
        self.io_registrations
            .borrow_mut()
            .insert(token, IoRegistration { waker, interest });

        Ok(token)
    }

    /// Deregister a file descriptor from I/O events
    #[cfg(not(feature = "tokio"))]
    pub fn deregister_io(&self, fd: RawFd, token: Token) -> std::io::Result<()> {
        self.io_selector.borrow_mut().deregister(fd)?;
        self.io_registrations.borrow_mut().remove(&token);
        Ok(())
    }

    /// Enqueue a task into the ready queue.
    fn enqueue(self: &Rc<Self>, task: Rc<RefCell<Task>>) {
        self.ready_queue.borrow_mut().push_back(task);
    }

    /// Spawn schedules an Awaitable as a task.
    ///
    /// The task runs concurrently with other tasks on the loop. Returns
    /// immediately; the result is delivered via on_done callback.
    pub fn spawn<A, F>(self: &Rc<Self>, awaitable: A, on_done: F)
    where
        A: Awaitable + 'static,
        F: FnOnce(PollResult) + 'static,
    {
        let task = Rc::new(RefCell::new(Task {
            awaitable: Rc::new(awaitable),
            on_done: Some(Box::new(on_done)),
        }));
        self.enqueue(task);
    }

    /// Run blocks until the given Awaitable completes, returning its result.
    ///
    /// This is the top-level entry for a script's root awaitable. The loop
    /// processes tasks until the given awaitable settles.
    pub fn run<A>(self: &Rc<Self>, awaitable: A) -> PollResult
    where
        A: Awaitable + 'static,
    {
        let result = Rc::new(RefCell::new(None));
        let result_for_callback = result.clone();

        let task = Rc::new(RefCell::new(Task {
            awaitable: Rc::new(awaitable),
            on_done: Some(Box::new(move |r| {
                *result_for_callback.borrow_mut() = Some(r);
            })),
        }));

        self.enqueue(task);

        // Drive the loop until result is ready
        while result.borrow().is_none() {
            // Process one task from the ready queue
            let task = {
                let mut queue = self.ready_queue.borrow_mut();
                queue.pop_front()
            };

            if let Some(task_rc) = task {
                self.poll_task(task_rc);
            } else {
                // Queue is empty - intelligently wait for next event
                self.wait_for_events();
            }
        }

        // Extract result without creating temporary borrow
        let final_result = result.borrow().clone();
        final_result.unwrap_or(PollResult::Pending)
    }

    /// Wait for events when the ready queue is empty.
    ///
    /// This method checks for pending timers and I/O events, sleeping until
    /// the next event is ready.
    fn wait_for_events(&self) {
        // Tick the timer wheel to fire any ready timers
        self.timer_wheel.borrow_mut().tick();

        // Check if there are now tasks in the queue after ticking
        if !self.ready_queue.borrow().is_empty() {
            return;
        }

        // Calculate sleep duration based on next timer
        let next_deadline = self.timer_wheel.borrow().next_deadline();

        let sleep_duration = match next_deadline {
            Some(deadline) => {
                let now = Instant::now();
                if deadline > now {
                    // Sleep until next timer
                    deadline.duration_since(now)
                } else {
                    // Timer already expired, don't sleep
                    return;
                }
            }
            None => {
                // No pending timers, sleep a bit to avoid busy-waiting
                Duration::from_millis(10)
            }
        };

        // Cap sleep duration to avoid blocking too long
        let capped_duration = sleep_duration.min(Duration::from_millis(100));

        // Check for I/O events (only when not using tokio)
        #[cfg(not(feature = "tokio"))]
        {
            let mut events = Vec::new();
            match self
                .io_selector
                .borrow_mut()
                .select(&mut events, Some(capped_duration))
            {
                Ok(_) => {
                    // Process I/O events
                    for event in events {
                        if let Some(registration) =
                            self.io_registrations.borrow().get(&event.token())
                        {
                            // Wake up the awaitable waiting for this I/O
                            (registration.waker)();
                        }
                    }
                }
                Err(e) => {
                    // I/O error - log and continue
                    eprintln!("I/O selector error: {}", e);
                }
            }
        }

        // Fallback sleep for tokio build
        #[cfg(feature = "tokio")]
        std::thread::sleep(capped_duration);
    }

    /// Drive the event loop until the result is ready.
    /// This is now integrated into the run method above.
    #[allow(dead_code)]
    fn run_until_complete_old(self: &Rc<Self>, result: &Rc<RefCell<Option<PollResult>>>) {
        while result.borrow().is_none() {
            // Process one task from the ready queue
            let task = {
                let mut queue = self.ready_queue.borrow_mut();
                queue.pop_front()
            };

            if let Some(task_rc) = task {
                self.poll_task(task_rc);
            } else {
                // Queue is empty but result not ready - this shouldn't happen
                // in a well-formed program, but we'll yield briefly
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    /// Poll a single task.
    fn poll_task(self: &Rc<Self>, task_rc: Rc<RefCell<Task>>) {
        let self_clone = self.clone();
        let task_clone = task_rc.clone();

        // Create a waker that re-enqueues this task
        let waker: Waker = Rc::new(move || {
            self_clone.enqueue(task_clone.clone());
        });

        // Poll the awaitable
        let poll_result = {
            let task = task_rc.borrow();
            task.awaitable.poll(waker)
        };

        // Handle the result
        match poll_result {
            PollResult::Ready(_) | PollResult::Rejected(_) => {
                // Task completed - invoke callback
                let mut task = task_rc.borrow_mut();
                if let Some(on_done) = task.on_done.take() {
                    on_done(poll_result);
                }
            }
            PollResult::Pending => {
                // Task is pending - waker has been registered, nothing to do
            }
        }
    }

    /// Run until all spawned tasks complete (quiescent state).
    ///
    /// Used by the host to drain the loop before exit.
    pub fn run_until_quiescent(self: &Rc<Self>) {
        while !self.ready_queue.borrow().is_empty() {
            let task = self.ready_queue.borrow_mut().pop_front();
            if let Some(task_rc) = task {
                self.poll_task(task_rc);
            }
        }
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        EventLoop {
            ready_queue: RefCell::new(VecDeque::new()),
            timer_wheel: Rc::new(RefCell::new(TimerWheel::new_unwrapped())),
            #[cfg(not(feature = "tokio"))]
            io_selector: RefCell::new(
                super::io_selector::new_selector().expect("Failed to create I/O selector"),
            ),
            #[cfg(not(feature = "tokio"))]
            io_registrations: RefCell::new(HashMap::new()),
            #[cfg(not(feature = "tokio"))]
            next_token: RefCell::new(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct TestAwaitable {
        ready: RefCell<bool>,
    }

    impl Awaitable for TestAwaitable {
        fn poll(&self, _waker: Waker) -> PollResult {
            if *self.ready.borrow() {
                PollResult::Ready(crate::object::Object::Number(42.0))
            } else {
                *self.ready.borrow_mut() = true;
                PollResult::Pending
            }
        }
    }

    #[test]
    fn test_event_loop_basic() {
        let loop_rc = EventLoop::new();
        let awaitable = TestAwaitable {
            ready: RefCell::new(true),
        };

        let result = loop_rc.run(awaitable);
        match result {
            PollResult::Ready(obj) => {
                if let crate::object::Object::Number(n) = obj {
                    assert_eq!(n, 42.0);
                } else {
                    panic!("Expected Number");
                }
            }
            _ => panic!("Expected Ready"),
        }
    }

    #[test]
    fn test_timer_wheel_integration() {
        let loop_rc = EventLoop::new();
        let timer_wheel = loop_rc.timer_wheel();

        // Schedule a timer
        let waker_called = Rc::new(RefCell::new(false));
        let waker_called_clone = waker_called.clone();
        let waker: Waker = Rc::new(move || {
            *waker_called_clone.borrow_mut() = true;
        });

        timer_wheel
            .borrow_mut()
            .schedule(Duration::from_millis(1), waker);

        // Wait for timer to fire
        std::thread::sleep(Duration::from_millis(10));
        timer_wheel.borrow_mut().tick();

        assert!(*waker_called.borrow());
    }

    #[test]
    fn test_next_deadline() {
        let loop_rc = EventLoop::new();
        let timer_wheel = loop_rc.timer_wheel();

        // No timers scheduled
        assert!(timer_wheel.borrow().next_deadline().is_none());

        // Schedule a timer
        let waker: Waker = Rc::new(|| {});
        timer_wheel
            .borrow_mut()
            .schedule(Duration::from_millis(100), waker);

        // Should have a deadline now
        assert!(timer_wheel.borrow().next_deadline().is_some());
    }
}
