use crate::{Interest, REACTOR};
use std::{
    os::unix::io::RawFd,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::Acquire},
        Arc,
    },
    task::{Context, Poll, Waker},
};

/// Future that waits for certain condition on file descriptor.
pub struct FdFuture {
    fd: RawFd,
    interest: Interest,
    last_waker: Option<Waker>,
    completed: Arc<AtomicBool>,
}

impl FdFuture {
    /// Creates new future.
    pub fn new(fd: RawFd, interest: Interest) -> FdFuture {
        FdFuture {
            fd,
            interest,
            last_waker: None,
            completed: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl std::future::Future for FdFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed.load(Acquire) {
            // wait completed
            REACTOR.unregister(self.fd);
            return Poll::Ready(());
        }

        if let Some(last_waker) = &self.last_waker {
            if last_waker.will_wake(cx.waker()) {
                // we are polled with the same waker, no need to re-register.
                return Poll::Pending;
            }
        }

        // waker has changed, we need to re-register
        REACTOR.unregister(self.fd);
        REACTOR.register(
            self.fd,
            self.interest,
            self.completed.clone(),
            cx.waker().clone(),
        );
        self.last_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
