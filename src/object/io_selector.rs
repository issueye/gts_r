/// I/O Multiplexing abstraction for async I/O operations
///
/// This module provides a cross-platform abstraction over OS-level I/O
/// multiplexing primitives:
/// - Linux: epoll
/// - macOS/BSD: kqueue
/// - Windows: IOCP (I/O Completion Ports)
///
/// The abstraction allows the EventLoop to efficiently wait for I/O
/// readiness events without busy-polling.
use std::io;
use std::time::Duration;

#[cfg(target_os = "linux")]
mod epoll;
#[cfg(target_os = "linux")]
pub use epoll::EpollSelector as Selector;

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
mod kqueue;
#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
pub use kqueue::KqueueSelector as Selector;

#[cfg(target_os = "windows")]
mod iocp;
#[cfg(target_os = "windows")]
pub use iocp::IocpSelector as Selector;

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "windows"
)))]
mod poll;
#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "windows"
)))]
pub use poll::PollSelector as Selector;

/// Token to identify I/O events
///
/// Each registered file descriptor gets a unique token that is
/// returned when the I/O becomes ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub usize);

/// I/O event interests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest {
    bits: u8,
}

impl Interest {
    /// Interest in readable events
    pub const READABLE: Interest = Interest { bits: 0b001 };

    /// Interest in writable events
    pub const WRITABLE: Interest = Interest { bits: 0b010 };

    /// Interest in both readable and writable events
    pub const READWRITE: Interest = Interest { bits: 0b011 };

    /// Check if readable
    pub fn is_readable(self) -> bool {
        self.bits & 0b001 != 0
    }

    /// Check if writable
    pub fn is_writable(self) -> bool {
        self.bits & 0b010 != 0
    }

    /// Add readable interest
    pub fn add_readable(self) -> Self {
        Interest {
            bits: self.bits | 0b001,
        }
    }

    /// Add writable interest
    pub fn add_writable(self) -> Self {
        Interest {
            bits: self.bits | 0b010,
        }
    }
}

/// I/O ready event
#[derive(Debug, Clone, Copy)]
pub struct Event {
    token: Token,
    readable: bool,
    writable: bool,
}

impl Event {
    /// Create a new event
    pub fn new(token: Token, readable: bool, writable: bool) -> Self {
        Event {
            token,
            readable,
            writable,
        }
    }

    /// Get the token for this event
    pub fn token(&self) -> Token {
        self.token
    }

    /// Check if readable
    pub fn is_readable(&self) -> bool {
        self.readable
    }

    /// Check if writable
    pub fn is_writable(&self) -> bool {
        self.writable
    }
}

/// I/O selector trait
///
/// Implementors provide platform-specific I/O multiplexing.
pub trait IoSelector {
    /// Register a file descriptor for I/O events
    fn register(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;

    /// Deregister a file descriptor
    fn deregister(&mut self, fd: RawFd) -> io::Result<()>;

    /// Modify the interest for a registered file descriptor
    fn reregister(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;

    /// Wait for I/O events, blocking up to the specified timeout
    ///
    /// Returns the number of events that occurred.
    /// Events can be retrieved via poll().
    fn select(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize>;
}

// Platform-specific raw file descriptor types
#[cfg(unix)]
pub type RawFd = std::os::unix::io::RawFd;

#[cfg(windows)]
pub type RawFd = std::os::windows::io::RawSocket;

/// Create a new I/O selector for the current platform
pub fn new_selector() -> io::Result<Selector> {
    Selector::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interest() {
        let interest = Interest::READABLE;
        assert!(interest.is_readable());
        assert!(!interest.is_writable());

        let interest = Interest::WRITABLE;
        assert!(!interest.is_readable());
        assert!(interest.is_writable());

        let interest = Interest::READWRITE;
        assert!(interest.is_readable());
        assert!(interest.is_writable());
    }

    #[test]
    fn test_interest_add() {
        let interest = Interest::READABLE.add_writable();
        assert!(interest.is_readable());
        assert!(interest.is_writable());
    }

    #[test]
    fn test_event() {
        let event = Event::new(Token(42), true, false);
        assert_eq!(event.token(), Token(42));
        assert!(event.is_readable());
        assert!(!event.is_writable());
    }
}
