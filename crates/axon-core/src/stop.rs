use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;

/// A cloneable cooperative cancellation handle — the global "hold everything"
/// brake (the STN hyperdirect pathway).
///
/// All clones share one flag, so calling [`stop`](Self::stop) on any clone halts
/// every runtime watching it. The synchronous [`Runtime`](crate::Runtime) checks
/// it at step boundaries (the in-flight step finishes, then the run halts). The
/// [`AsyncRuntime`](crate::AsyncRuntime) additionally registers a waker, so
/// `stop` cancels an awaiting module *mid-future* — a hung tool is dropped, not
/// waited on.
#[derive(Debug, Clone, Default)]
pub struct StopToken {
    stopped: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl StopToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. Idempotent; visible to every clone, and wakes any
    /// run currently awaiting on this token so it can cancel promptly.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.waker.lock() {
            if let Some(waker) = slot.take() {
                waker.wake();
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Register the waker of an awaiting run so [`stop`](Self::stop) can wake it
    /// for mid-future cancellation.
    pub(crate) fn register(&self, waker: &Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker.clone());
        }
    }
}
