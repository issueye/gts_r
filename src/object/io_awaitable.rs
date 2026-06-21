/// I/O Awaitable implementations for async network operations
///
/// This module provides Awaitable wrappers for common I/O operations:
/// - TcpConnect: Async TCP connection establishment
/// - TcpRead: Async reading from TCP streams
/// - TcpWrite: Async writing to TCP streams
///
/// These use non-blocking I/O with the native event loop. When the `tokio`
/// feature is enabled, these will delegate to tokio's async I/O primitives.
use crate::ast::Position;
use crate::object::{new_error, num_obj, ArrayData, Awaitable, Object, PollResult, Waker};
use std::cell::RefCell;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::rc::Rc;

/// Awaitable for TCP connection establishment
///
/// Attempts to connect to the given address asynchronously.
/// The connection is performed in non-blocking mode.
pub struct TcpConnectAwaitable {
    addr: String,
    stream: RefCell<Option<Result<TcpStream, io::Error>>>,
    waker: RefCell<Option<Waker>>,
}

impl TcpConnectAwaitable {
    /// Create a new TCP connect awaitable
    pub fn new(addr: impl Into<String>) -> Rc<Self> {
        Rc::new(TcpConnectAwaitable {
            addr: addr.into(),
            stream: RefCell::new(None),
            waker: RefCell::new(None),
        })
    }

    /// Attempt the connection in non-blocking mode
    fn try_connect(&self) -> Result<TcpStream, io::Error> {
        let stream = TcpStream::connect(&self.addr)?;
        stream.set_nonblocking(true)?;
        Ok(stream)
    }
}

impl Awaitable for TcpConnectAwaitable {
    fn poll(&self, waker: Waker) -> PollResult {
        // Check if we already attempted the connection
        if self.stream.borrow().is_none() {
            // First poll - attempt connection
            let result = self.try_connect();
            *self.stream.borrow_mut() = Some(result);
            *self.waker.borrow_mut() = Some(waker.clone());
        }

        // Check connection result
        match self.stream.borrow_mut().take() {
            Some(Ok(stream)) => {
                // Connection succeeded
                // TODO: Wrap TcpStream in a GTS object
                // For now, just return success marker
                PollResult::Ready(num_obj(stream.as_raw_fd() as f64))
            }
            Some(Err(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                // Connection in progress - restore state and return pending
                *self.stream.borrow_mut() = Some(Err(e));

                // NOTE: In a real implementation, this would register with epoll/kqueue/IOCP
                // For now, the event loop will re-poll this awaitable later
                // The waker would be invoked by the I/O readiness system

                PollResult::Pending
            }
            Some(Err(e)) => {
                // Connection failed
                PollResult::Rejected(new_error(
                    Position::default(),
                    format!("TcpConnectError: {}", e),
                ))
            }
            None => {
                // Should not happen
                PollResult::Rejected(new_error(
                    Position::default(),
                    "TcpConnectError: invalid state",
                ))
            }
        }
    }
}

/// Awaitable for TCP read operations
///
/// Reads data from a TCP stream asynchronously.
pub struct TcpReadAwaitable {
    stream: Rc<RefCell<TcpStream>>,
    buffer_size: usize,
    result: RefCell<Option<Result<Vec<u8>, io::Error>>>,
}

impl TcpReadAwaitable {
    /// Create a new TCP read awaitable
    pub fn new(stream: Rc<RefCell<TcpStream>>, buffer_size: usize) -> Rc<Self> {
        Rc::new(TcpReadAwaitable {
            stream,
            buffer_size,
            result: RefCell::new(None),
        })
    }

    fn try_read(&self) -> Result<Vec<u8>, io::Error> {
        let mut buffer = vec![0u8; self.buffer_size];
        let mut stream = self.stream.borrow_mut();

        match stream.read(&mut buffer) {
            Ok(n) => {
                buffer.truncate(n);
                Ok(buffer)
            }
            Err(e) => Err(e),
        }
    }
}

impl Awaitable for TcpReadAwaitable {
    fn poll(&self, waker: Waker) -> PollResult {
        if self.result.borrow().is_none() {
            let result = self.try_read();
            *self.result.borrow_mut() = Some(result);
        }

        match self.result.borrow_mut().take() {
            Some(Ok(data)) => {
                // Convert bytes to array of integers
                let elements: Vec<Object> = data.iter().map(|&b| num_obj(b as f64)).collect();

                PollResult::Ready(Object::Array(Rc::new(RefCell::new(ArrayData { elements }))))
            }
            Some(Err(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                *self.result.borrow_mut() = Some(Err(e));

                // NOTE: In a real implementation, this would register with epoll/kqueue/IOCP
                // For now, the event loop will re-poll this awaitable later

                PollResult::Pending
            }
            Some(Err(e)) => PollResult::Rejected(new_error(
                Position::default(),
                format!("TcpReadError: {}", e),
            )),
            None => PollResult::Rejected(new_error(
                Position::default(),
                "TcpReadError: invalid state",
            )),
        }
    }
}

/// Awaitable for TCP write operations
///
/// Writes data to a TCP stream asynchronously.
pub struct TcpWriteAwaitable {
    stream: Rc<RefCell<TcpStream>>,
    data: Vec<u8>,
    result: RefCell<Option<Result<usize, io::Error>>>,
}

impl TcpWriteAwaitable {
    /// Create a new TCP write awaitable
    pub fn new(stream: Rc<RefCell<TcpStream>>, data: Vec<u8>) -> Rc<Self> {
        Rc::new(TcpWriteAwaitable {
            stream,
            data,
            result: RefCell::new(None),
        })
    }

    fn try_write(&self) -> Result<usize, io::Error> {
        let mut stream = self.stream.borrow_mut();
        stream.write(&self.data)
    }
}

impl Awaitable for TcpWriteAwaitable {
    fn poll(&self, waker: Waker) -> PollResult {
        if self.result.borrow().is_none() {
            let result = self.try_write();
            *self.result.borrow_mut() = Some(result);
        }

        match self.result.borrow_mut().take() {
            Some(Ok(n)) => PollResult::Ready(num_obj(n as f64)),
            Some(Err(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                *self.result.borrow_mut() = Some(Err(e));

                // NOTE: In a real implementation, this would register with epoll/kqueue/IOCP
                // For now, the event loop will re-poll this awaitable later

                PollResult::Pending
            }
            Some(Err(e)) => PollResult::Rejected(new_error(
                Position::default(),
                format!("TcpWriteError: {}", e),
            )),
            None => PollResult::Rejected(new_error(
                Position::default(),
                "TcpWriteError: invalid state",
            )),
        }
    }
}

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(windows)]
use std::os::windows::io::AsRawSocket;

#[cfg(unix)]
trait RawFdProvider {
    fn as_raw_fd(&self) -> i32;
}

#[cfg(unix)]
impl RawFdProvider for TcpStream {
    fn as_raw_fd(&self) -> i32 {
        AsRawFd::as_raw_fd(self)
    }
}

#[cfg(windows)]
trait RawFdProvider {
    fn as_raw_fd(&self) -> i64;
}

#[cfg(windows)]
impl RawFdProvider for TcpStream {
    fn as_raw_fd(&self) -> i64 {
        AsRawSocket::as_raw_socket(self) as i64
    }
}
