use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cloneable cooperative cancellation handle — the global "hold everything"
/// brake (the STN hyperdirect pathway).
///
/// All clones share one flag, so calling [`stop`](Self::stop) on any clone halts
/// every runtime watching it. Runtimes check it at step boundaries, so it is
/// cooperative: the in-flight module (or tool) finishes its current step, then
/// the run halts before the next one — the user-pressed-Ctrl-C / "stop, that
/// command is destructive" path for an agent dispatcher.
#[derive(Debug, Clone, Default)]
pub struct StopToken {
    stopped: Arc<AtomicBool>,
}

impl StopToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. Idempotent; visible to every clone.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}
