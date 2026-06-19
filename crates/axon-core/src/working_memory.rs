/// A bounded, typed working-memory scratchpad: a small set of named slots the
/// caller can UPDATE as evidence arrives and HOLD across steps so they survive
/// eviction.
///
/// This is the dlPFC working-memory function the routed loop otherwise lacks —
/// it is stateless between steps. It is distinct from episodic memory (long-term
/// recall) and the workspace (a broadcast bus): this is the protected, actively
/// maintained *task context*. The HOLD/UPDATE gate mirrors the basal-ganglia
/// gate that decides what stays active versus what is overwritten.
#[derive(Debug, Clone)]
pub struct WorkingMemory<T> {
    capacity: usize,
    slots: Vec<Slot<T>>,
}

#[derive(Debug, Clone)]
struct Slot<T> {
    key: String,
    value: T,
    held: bool,
}

impl<T> WorkingMemory<T> {
    /// Create a scratchpad bounded to `capacity` slots (clamped to at least 1).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            slots: Vec::new(),
        }
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.slots.iter().position(|slot| slot.key == key)
    }

    /// UPDATE: write `value` under `key`. An existing slot is overwritten in
    /// place (keeping its hold). A new key at capacity evicts the oldest *unheld*
    /// slot; if every slot is held the write is refused, since held context is
    /// protected. Returns whether the value is now stored.
    pub fn update(&mut self, key: impl Into<String>, value: T) -> bool {
        let key = key.into();
        if let Some(index) = self.position(&key) {
            self.slots[index].value = value;
            return true;
        }
        if self.slots.len() >= self.capacity {
            match self.slots.iter().position(|slot| !slot.held) {
                Some(victim) => {
                    self.slots.remove(victim);
                }
                None => return false,
            }
        }
        self.slots.push(Slot {
            key,
            value,
            held: false,
        });
        true
    }

    /// HOLD: protect a slot from eviction. Returns whether the key exists.
    pub fn hold(&mut self, key: &str) -> bool {
        self.set_held(key, true)
    }

    /// Release a held slot back to being evictable. Returns whether it exists.
    pub fn release(&mut self, key: &str) -> bool {
        self.set_held(key, false)
    }

    fn set_held(&mut self, key: &str, held: bool) -> bool {
        match self.position(key) {
            Some(index) => {
                self.slots[index].held = held;
                true
            }
            None => false,
        }
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.position(key).map(|index| &self.slots[index].value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        match self.position(key) {
            Some(index) => Some(&mut self.slots[index].value),
            None => None,
        }
    }

    pub fn is_held(&self, key: &str) -> bool {
        self.position(key)
            .is_some_and(|index| self.slots[index].held)
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        self.position(key)
            .map(|index| self.slots.remove(index).value)
    }

    /// Iterate `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.slots
            .iter()
            .map(|slot| (slot.key.as_str(), &slot.value))
    }
}
