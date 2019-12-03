//! A reactor for handling file descriptors in a background thread.
//!
//! ## Implementation Notes
//!
//! - The reactor's background thread is spawned on the first time that the reactor handle is fetched.
//! - Each file descriptor registers an interest to listen for.
//! - On registering a new file descriptor, a pipe is used to interrupt the poll operation.

use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Write},
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::Waker,
};

type ReactorFds = Arc<Mutex<HashMap<RawFd, (Interest, Arc<AtomicBool>, Waker)>>>;

bitflags::bitflags! {
    /// Events that should be listened for on a given file descriptor.
    pub struct Interest: i16 {
        /// Listen for read events
        const READ = libc::POLLIN;

        /// Listen for write events
        const WRITE = libc::POLLOUT;

        /// Listen for both read and write events
        const BOTH = libc::POLLIN | libc::POLLOUT;
    }
}

/// A handle to the reactor, for registering and unregistering file descriptors.
pub struct Handle {
    /// A set of file descriptors which are currently registered on the reactor.
    fds: ReactorFds,

    /// The write end of the pipe, for interrupting the poll operation.
    interrupt: File,
}

impl Handle {
    /// Register a new file descriptor onto the reactor.
    pub fn register(
        &self,
        fd: RawFd,
        interest: Interest,
        completed: Arc<AtomicBool>,
        waker: Waker,
    ) {
        let mut lock = self.fds.lock().unwrap();
        lock.insert(fd, (interest, completed, waker));
        let _ = self.interrupt.try_clone().unwrap().write_all(b"0");
    }

    /// Unregister the given file descriptor from the reactor.
    pub fn unregister(&self, fd: RawFd) {
        let mut lock = self.fds.lock().unwrap();
        lock.remove(&fd);
        let _ = self.interrupt.try_clone().unwrap().write_all(b"0");
    }
}

/// Fetches the handle to the reactor which is running in a background thread.
pub static REACTOR: Lazy<Handle> = Lazy::new(|| {
    // Create a pipe to use as an interruption mechanism.
    let (mut reader, writer) = create_pipe();

    let fds: ReactorFds = Arc::default();
    let fds_ = fds.clone();

    std::thread::spawn(move || {
        let fds = fds_;
        let mut pollers = Vec::new();
        let mut buffer = [0u8; 1];

        pollers.push(libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });

        loop {
            let returned = unsafe {
                let pollers: &mut [libc::pollfd] = &mut pollers;
                libc::poll(
                    pollers as *mut _ as *mut libc::pollfd,
                    pollers.len() as u64,
                    -1,
                )
            };

            if returned == -1 {
                panic!(
                    "fatal error in process reactor: {}",
                    io::Error::last_os_error()
                );
            } else if returned < 1 {
                continue;
            }

            let lock = fds.lock().unwrap();
            if pollers[0].revents == libc::POLLIN {
                let _ = reader.read(&mut buffer);
            } else {
                pollers[1..]
                    .iter()
                    .filter(|event| event.revents != 0)
                    .for_each(|event| {
                        if let Some(value) = lock.get(&event.fd) {
                            if value
                                .0
                                .contains(Interest::from_bits_truncate(event.revents))
                            {
                                value.1.store(true, Ordering::SeqCst);
                                value.2.wake_by_ref();
                            }
                        }
                    })
            }

            pollers.clear();

            pollers.push(libc::pollfd {
                fd: reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });

            for (&fd, &(interest, _, _)) in lock.iter() {
                pollers.push(libc::pollfd {
                    fd,
                    events: interest.bits(),
                    revents: 0,
                });
            }
        }
    });

    Handle {
        fds,
        interrupt: writer,
    }
});

fn create_pipe() -> (File, File) {
    let mut fds = [0; 2];
    unsafe { libc::pipe(&mut fds as *mut _ as *mut libc::c_int) };
    let reader = unsafe { File::from_raw_fd(fds[0]) };
    let writer = unsafe { File::from_raw_fd(fds[1]) };
    (reader, writer)
}
