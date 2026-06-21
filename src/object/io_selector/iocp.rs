/// Windows IOCP-based I/O selector
///
/// This module provides an I/O multiplexing implementation using Windows'
/// I/O Completion Ports (IOCP), which is the most efficient way to handle
/// thousands of concurrent connections on Windows.

#[cfg(windows)]
use winapi::shared::minwindef::FALSE;
#[cfg(windows)]
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use winapi::um::ioapiset::{CreateIoCompletionPort, GetQueuedCompletionStatusEx};
#[cfg(windows)]
use winapi::um::minwinbase::OVERLAPPED_ENTRY;
#[cfg(windows)]
use winapi::um::winbase::INFINITE;
#[cfg(windows)]
use winapi::um::winnt::HANDLE;

use super::{Event, Interest, IoSelector, RawFd, Token};
use std::io;
use std::time::Duration;

/// IOCP-based selector
pub struct IocpSelector {
    #[cfg(windows)]
    iocp_handle: HANDLE,
    #[cfg(not(windows))]
    _phantom: (),
}

impl IocpSelector {
    /// Create a new IOCP selector
    pub fn new() -> io::Result<Self> {
        #[cfg(windows)]
        {
            let iocp_handle =
                unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 0) };

            if iocp_handle.is_null() {
                return Err(io::Error::last_os_error());
            }

            Ok(IocpSelector { iocp_handle })
        }

        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "IOCP is only available on Windows",
            ))
        }
    }
}

impl IoSelector for IocpSelector {
    fn register(&mut self, fd: RawFd, token: Token, _interest: Interest) -> io::Result<()> {
        #[cfg(windows)]
        {
            let result =
                unsafe { CreateIoCompletionPort(fd as HANDLE, self.iocp_handle, token.0, 0) };

            if result.is_null() {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }

        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "IOCP is only available on Windows",
            ))
        }
    }

    fn deregister(&mut self, _fd: RawFd) -> io::Result<()> {
        // IOCP automatically deregisters when the socket is closed
        // No explicit deregistration needed
        Ok(())
    }

    fn reregister(&mut self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        // For IOCP, we can just re-associate with a new token
        self.register(fd, token, interest)
    }

    fn select(&mut self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize> {
        #[cfg(windows)]
        {
            let timeout_ms = timeout
                .map(|d| d.as_millis().min(u32::MAX as u128) as u32)
                .unwrap_or(INFINITE);

            const MAX_EVENTS: usize = 1024;
            let mut entries: [OVERLAPPED_ENTRY; MAX_EVENTS] = unsafe { std::mem::zeroed() };
            let mut removed: u32 = 0;

            let result = unsafe {
                GetQueuedCompletionStatusEx(
                    self.iocp_handle,
                    entries.as_mut_ptr(),
                    MAX_EVENTS as u32,
                    &mut removed,
                    timeout_ms,
                    FALSE,
                )
            };

            if result == 0 {
                let err = io::Error::last_os_error();
                // Timeout is not an error
                if err.kind() == io::ErrorKind::TimedOut {
                    events.clear();
                    return Ok(0);
                }
                return Err(err);
            }

            events.clear();
            for i in 0..removed as usize {
                let entry = entries[i];
                let token = Token(entry.lpCompletionKey);

                // On Windows, we assume both read and write are ready
                // This is a simplification - proper IOCP usage would track
                // operation types separately
                events.push(Event::new(token, true, true));
            }

            Ok(removed as usize)
        }

        #[cfg(not(windows))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "IOCP is only available on Windows",
            ))
        }
    }
}

impl Drop for IocpSelector {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            CloseHandle(self.iocp_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_create_selector() {
        let selector = IocpSelector::new();
        assert!(selector.is_ok());
    }
}
