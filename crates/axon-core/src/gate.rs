use std::fmt;

use crate::signal::{Priority, Signal};

pub trait Gate<P>: fmt::Debug {
    fn admits(&self, signal: &Signal<P>) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Allow;

impl<P> Gate<P> for Allow {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DropSignal;

impl<P> Gate<P> for DropSignal {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MinPriority {
    minimum: Priority,
}

impl MinPriority {
    pub const fn new(minimum: Priority) -> Self {
        Self { minimum }
    }

    pub const fn minimum(self) -> Priority {
        self.minimum
    }
}

impl<P> Gate<P> for MinPriority {
    fn admits(&self, signal: &Signal<P>) -> bool {
        signal.priority() >= self.minimum
    }
}
