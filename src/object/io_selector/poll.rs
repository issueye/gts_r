/// Fallback poll-based I/O selector
///
/// This module provides a simple polling-based I/O selector for platforms
/// that don't have epoll, kqueue, or IOCP. This is less efficient but
/// portable to any POSIX system.
use super::{Event, Interest, IoSelector, RawFd, Token};
use std::collections::HashMap;
use std::io;
use std::time::Duration;

struct Registration {
    token: Token,
    interest: Interest,
}

/// Simple poll-based selector
pub struct PollSelector {
    registrations: HashMap<RawFd, Registration>,
}

impl PollSelector {
    /// Create a new poll selector
    pub fn new() -> io::Result<Self> {
        Ok(PollSelector {
            registrations: HashMap::new(),
        })
    }

    fn build_pollfds(&self) -> Vec<libc::pollfd> {
        self.registrations
            .iter()
            .map(|(&fd, reg)| {
                let mut events = 0i16;
                if reg.interest.is_readable() {
                    events |= libc::POLLIN | libc::POLLRDHUP;
                }
                if reg.interest.is_writable() {
                    events |= libc::POLLOUT;
                }

                libc::pollfd {
                    fd,
                    events,
                    revents: 0,
                }
            })
            .collect()
    }
}

impl IoSelector for PollSelector {
    fn register(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        if self.registrations.contains_key(&fd) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "fd already registered",
            ));
        }

        self.registrations
            .insert(fd, Registration { token, interest });
        Ok(())
    }

    fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        self.registrations.remove(&fd);
        Ok(())
    }

    fn reregister(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        if let Some(reg) = self.registrations.get_mut(&fd) {
            reg.token = token;
            reg.interest = interest;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "fd not registered"))
        }
    }

    fn select(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize> {
        if self.registrations.is_empty() {
            events.clear();

            // Still need to sleep for the timeout period
            if let Some(duration) = timeout {
                std::thread::sleep(duration);
            }

            return Ok(0);
        }

        let timeout_ms = timeout
            .map(|d| d.as_millis().min(i32::MAX as u128) as i32)
            .unwrap_or(-1);

        let mut pollfds = self.build_pollfds();
        let fd_map: HashMap<RawFd, Token> = self
            .registrations
            .iter()
            .map(|(&fd, reg)| (fd, reg.token))
            .collect();

        let result = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as u64, timeout_ms) };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        events.clear();
        for pollfd in pollfds {
            if pollfd.revents != 0 {
                if let Some(&token) = fd_map.get(&pollfd.fd) {
                    let readable = (pollfd.revents & (libc::POLLIN | libc::POLLRDHUP)) != 0;
                    let writable = (pollfd.revents & libc::POLLOUT) != 0;
                    events.push(Event::new(token, readable, writable));
                }
            }
        }

        Ok(events.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_selector() {
        let selector = PollSelector::new();
        assert!(selector.is_ok());
    }

    #[test]
    fn test_register_deregister() {
        let mut selector = PollSelector::new().unwrap();

        // Create a pipe for testing
        let mut fds: [libc::c_int; 2] = [0; 2];
        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(result, 0);

        let read_fd = fds[0];

        // Register
        let result = selector.register(read_fd, Token(1), Interest::READABLE);
        assert!(result.is_ok());

        // Deregister
        let result = selector.deregister(read_fd);
        assert!(result.is_ok());

        // Clean up
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
    }

    #[test]
    fn test_select_with_timeout() {
        let mut selector = PollSelector::new().unwrap();
        let mut events = Vec::new();

        // Should timeout immediately with no registered fds
        let result = selector.select(&mut events, Some(Duration::from_millis(1)));
        assert!(result.is_ok());
        assert_eq!(events.len(), 0);
    }
}
