use std::fmt;

use crate::id::ModuleId;
use crate::module::{Module, ModuleError, ModuleOutput};
use crate::signal::Signal;

/// One dendritic branch: a predicate that decides whether this branch "fires"
/// for a signal.
type Branch<P> = Box<dyn FnMut(&Signal<P>) -> bool>;

/// A multi-compartment ("dendritic") module: a single routing node whose firing
/// is a *nonlinear* function of several independent branches, rather than a flat
/// pass-through.
///
/// A pyramidal neuron is not a simple function — its dendritic branches do local
/// coincidence detection, and the cell fires only when enough branches are
/// active together. `CompartmentModule` models that as a coincidence threshold:
/// it passes the signal on only when at least `threshold` branches fire,
/// otherwise it drops it. The routing core stays unaware — this is entirely a
/// module-internal nonlinearity, opt-in for module authors who want richer leaf
/// computation than `FnModule`.
pub struct CompartmentModule<P> {
    id: ModuleId,
    branches: Vec<Branch<P>>,
    threshold: usize,
}

impl<P> CompartmentModule<P> {
    /// A module that fires (passes the signal on) only when at least `threshold`
    /// of its branches fire.
    pub fn new(id: ModuleId, threshold: usize) -> Self {
        Self {
            id,
            branches: Vec::new(),
            threshold,
        }
    }

    /// Add a dendritic branch — a predicate over the incoming signal.
    #[must_use]
    pub fn with_branch<F>(mut self, branch: F) -> Self
    where
        F: FnMut(&Signal<P>) -> bool + 'static,
    {
        self.branches.push(Box::new(branch));
        self
    }

    pub fn branch_count(&self) -> usize {
        self.branches.len()
    }
}

impl<P> fmt::Debug for CompartmentModule<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompartmentModule")
            .field("id", &self.id)
            .field("branches", &self.branches.len())
            .field("threshold", &self.threshold)
            .finish()
    }
}

impl<P> Module<P> for CompartmentModule<P> {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, signal: Signal<P>) -> Result<ModuleOutput<P>, ModuleError> {
        let mut fired = 0;
        for branch in &mut self.branches {
            if branch(&signal) {
                fired += 1;
            }
        }
        // Coincidence detection: pass on only when enough branches agree.
        Ok(if fired >= self.threshold {
            ModuleOutput::emit_signal(signal)
        } else {
            ModuleOutput::drop()
        })
    }
}
