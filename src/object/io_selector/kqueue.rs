/// macOS/BSD kqueue-based I/O selector
///
/// This module provides an I/O multiplexing implementation using BSD's
/// kqueue API, which is the native event notification mechanism on
/// macOS and BSD systems.
use super::{Event, Interest, IoSelector, RawFd, Token};
use std::io;
use std::time::Duration;

/// kqueue-based selector
pub struct KqueueSelector {
    kqueue_fd: RawFd,
}

impl KqueueSelector {
    /// Create a new kqueue selector
    pub fn new() -> io::Result<Self> {
        let kqueue_fd = unsafe { libc::kqueue() };
        if kqueue_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(KqueueSelector { kqueue_fd })
    }

    fn create_kevent(fd: RawFd, token: Token, interest: Interest, flags: u16) -> Vec<libc::kevent> {
        let mut events = Vec::new();

        if interest.is_readable() {
            events.push(libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ,
                flags,
                fflags: 0,
                data: 0,
                udata: token.0 as *mut libc::c_void,
            });
        }

        if interest.is_writable() {
            events.push(libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_WRITE,
                flags,
                fflags: 0,
                data: 0,
                udata: token.0 as *mut libc::c_void,
            });
        }

        events
    }
}

impl IoSelector for KqueueSelector {
    fn register(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let kevents =
            Self::create_kevent(fd, token, interest, (libc::EV_ADD | libc::EV_CLEAR) as u16);

        let result = unsafe {
            libc::kevent(
                self.kqueue_fd,
                kevents.as_ptr(),
                kevents.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        // Deregister both read and write filters
        let mut kevents = vec![
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ,
                flags: libc::EV_DELETE as u16,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_WRITE,
                flags: libc::EV_DELETE as u16,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
        ];

        let result = unsafe {
            libc::kevent(
                self.kqueue_fd,
                kevents.as_mut_ptr(),
                kevents.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        // It's ok if one of the filters doesn't exist
        // We just want to ensure the fd is fully deregistered
        Ok(())
    }

    fn reregister(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        // For kqueue, we can just add again with EV_ADD
        self.register(fd, token, interest)
    }

    fn select(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize> {
        let timeout_spec = timeout.map(|d| libc::timespec {
            tv_sec: d.as_secs() as libc::time_t,
            tv_nsec: d.subsec_nanos() as libc::c_long,
        });

        const MAX_EVENTS: usize = 1024;
        let mut kevents: [libc::kevent; MAX_EVENTS] = unsafe { std::mem::zeroed() };

        let result = unsafe {
            libc::kevent(
                self.kqueue_fd,
                std::ptr::null(),
                0,
                kevents.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_spec
                    .as_ref()
                    .map(|t| t as *const libc::timespec)
                    .unwrap_or(std::ptr::null()),
            )
        };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        events.clear();

        // Group events by token (same fd may have both read and write events)
        let mut event_map = std::collections::HashMap::new();

        for i in 0..result as usize {
            let kevent = kevents[i];
            let token = Token(kevent.udata as usize);

            let entry = event_map.entry(token).or_insert((false, false));

            if kevent.filter == libc::EVFILT_READ {
                entry.0 = true;
            } else if kevent.filter == libc::EVFILT_WRITE {
                entry.1 = true;
            }
        }

        for (token, (readable, writable)) in event_map {
            events.push(Event::new(token, readable, writable));
        }

        Ok(events.len())
    }
}

impl Drop for KqueueSelector {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.kqueue_fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_selector() {
        let selector = KqueueSelector::new();
        assert!(selector.is_ok());
    }

    #[test]
    fn test_register_pipe() {
        let mut selector = KqueueSelector::new().unwrap();

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
        let mut selector = KqueueSelector::new().unwrap();
        let mut events = Vec::new();

        // Should timeout immediately with no registered fds
        let result = selector.select(&mut events, Some(Duration::from_millis(1)));
        assert!(result.is_ok());
        assert_eq!(events.len(), 0);
    }
}
