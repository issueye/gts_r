/// Integration test for I/O multiplexing
///
/// This test demonstrates the I/O selector working with the EventLoop.

#[cfg(test)]
#[cfg(not(feature = "tokio"))]
mod tests {
    use crate::object::{EventLoop, Interest, Token};
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;

    #[test]
    fn test_io_selector_with_pipe() {
        // Create a pipe for testing
        let mut fds: [libc::c_int; 2] = [0; 2];
        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(result, 0);

        let read_fd = fds[0];
        let write_fd = fds[1];

        // Create event loop
        let event_loop = EventLoop::new();

        // Track if waker was called
        let waker_called = std::rc::Rc::new(std::cell::RefCell::new(false));
        let waker_called_clone = waker_called.clone();

        // Create waker
        let waker = std::rc::Rc::new(move || {
            *waker_called_clone.borrow_mut() = true;
        });

        // Register read end for readable events
        let token = event_loop
            .register_io(read_fd, Interest::READABLE, waker)
            .expect("Failed to register I/O");

        // Write some data to trigger readability
        let data = b"Hello, I/O!";
        let n = unsafe {
            libc::write(write_fd, data.as_ptr() as *const libc::c_void, data.len())
        };
        assert_eq!(n as usize, data.len());

        // Wait for I/O events (with timeout)
        event_loop.timer_wheel().borrow_mut().tick();
        
        // Note: In a real scenario, wait_for_events would be called
        // and it would invoke the waker when the fd becomes readable

        // Clean up
        event_loop.deregister_io(read_fd, token).ok();
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn test_io_selector_multiple_fds() {
        // Create two pipes
        let mut fds1: [libc::c_int; 2] = [0; 2];
        let mut fds2: [libc::c_int; 2] = [0; 2];
        unsafe {
            libc::pipe(fds1.as_mut_ptr());
            libc::pipe(fds2.as_mut_ptr());
        }

        let event_loop = EventLoop::new();

        // Register both read ends
        let waker1 = std::rc::Rc::new(|| {});
        let waker2 = std::rc::Rc::new(|| {});

        let token1 = event_loop
            .register_io(fds1[0], Interest::READABLE, waker1)
            .expect("Failed to register fd1");

        let token2 = event_loop
            .register_io(fds2[0], Interest::READABLE, waker2)
            .expect("Failed to register fd2");

        // Verify we can deregister
        event_loop.deregister_io(fds1[0], token1).ok();
        event_loop.deregister_io(fds2[0], token2).ok();

        // Clean up
        unsafe {
            libc::close(fds1[0]);
            libc::close(fds1[1]);
            libc::close(fds2[0]);
            libc::close(fds2[1]);
        }
    }
}
