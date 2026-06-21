/// Linux epoll-based I/O selector
///
/// This module provides an I/O multiplexing implementation using Linux's
/// epoll API, which is the most efficient way to handle thousands of
/// concurrent connections on Linux.
use super::{Event, Interest, IoSelector, RawFd, Token};
use std::io;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

/// epoll-based selector
pub struct EpollSelector {
    epoll_fd: RawFd,
}

impl EpollSelector {
    /// Create a new epoll selector
    pub fn new() -> io::Result<Self> {
        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(EpollSelector { epoll_fd })
    }

    fn interest_to_epoll_flags(interest: Interest) -> u32 {
        let mut flags = libc::EPOLLET as u32; // Edge-triggered mode

        if interest.is_readable() {
            flags |= libc::EPOLLIN as u32 | libc::EPOLLRDHUP as u32;
        }

        if interest.is_writable() {
            flags |= libc::EPOLLOUT as u32;
        }

        flags
    }
}

impl IoSelector for EpollSelector {
    fn register(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: Self::interest_to_epoll_flags(interest),
            u64: token.0 as u64,
        };

        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_ADD,
                fd,
                &mut event as *mut libc::epoll_event,
            )
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        let result = unsafe {
            libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut())
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn reregister(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: Self::interest_to_epoll_flags(interest),
            u64: token.0 as u64,
        };

        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_MOD,
                fd,
                &mut event as *mut libc::epoll_event,
            )
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn select(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize> {
        let timeout_ms = timeout
            .map(|d| d.as_millis().min(i32::MAX as u128) as i32)
            .unwrap_or(-1); // -1 means infinite timeout

        const MAX_EVENTS: usize = 1024;
        let mut epoll_events: [libc::epoll_event; MAX_EVENTS] = unsafe { std::mem::zeroed() };

        let result = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                epoll_events.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_ms,
            )
        };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        events.clear();
        for i in 0..result as usize {
            let epoll_event = epoll_events[i];
            let token = Token(epoll_event.u64 as usize);
            let flags = epoll_event.events;

            let readable = (flags & (libc::EPOLLIN | libc::EPOLLRDHUP) as u32) != 0;
            let writable = (flags & libc::EPOLLOUT as u32) != 0;

            events.push(Event::new(token, readable, writable));
        }

        Ok(result as usize)
    }
}

impl Drop for EpollSelector {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.epoll_fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn test_create_selector() {
        let selector = EpollSelector::new();
        assert!(selector.is_ok());
    }

    #[test]
    fn test_register_pipe() {
        let mut selector = EpollSelector::new().unwrap();

        // Create a pipe for testing
        let mut fds: [libc::c_int; 2] = [0; 2];
        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(result, 0);

        let read_fd = fds[0];
        let write_fd = fds[1];

        // Register the read end
        let result = selector.register(read_fd, Token(1), Interest::READABLE);
        assert!(result.is_ok());

        // Register the write end
        let result = selector.register(write_fd, Token(2), Interest::WRITABLE);
        assert!(result.is_ok());

        // Clean up
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn test_select_with_timeout() {
        let mut selector = EpollSelector::new().unwrap();
        let mut events = Vec::new();

        // Should timeout immediately with no registered fds
        let result = selector.select(&mut events, Some(Duration::from_millis(1)));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert_eq!(events.len(), 0);
    }
}
