#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Priority(i16);

impl Priority {
    pub const fn new(value: i16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal<P> {
    payload: P,
    priority: Priority,
}

impl<P> Signal<P> {
    pub fn new(payload: P) -> Self {
        Self::with_priority(payload, Priority::default())
    }

    pub const fn with_priority(payload: P, priority: Priority) -> Self {
        Self { payload, priority }
    }

    pub const fn payload(&self) -> &P {
        &self.payload
    }

    pub const fn priority(&self) -> Priority {
        self.priority
    }

    pub fn into_payload(self) -> P {
        self.payload
    }

    pub fn map<Q>(self, transform: impl FnOnce(P) -> Q) -> Signal<Q> {
        Signal::with_priority(transform(self.payload), self.priority)
    }
}
