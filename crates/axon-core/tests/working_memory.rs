use std::error::Error;

use axon_core::WorkingMemory;

#[test]
fn update_writes_and_overwrites_in_place() -> Result<(), Box<dyn Error>> {
    // Given: a scratchpad with one slot written.
    let mut wm = WorkingMemory::new(4);
    assert!(wm.update("goal", "ship axon"));

    // When: the same key is updated.
    assert!(wm.update("goal", "ship axon v2"));

    // Then: it is overwritten in place, not duplicated.
    assert_eq!(wm.len(), 1);
    assert_eq!(wm.get("goal"), Some(&"ship axon v2"));
    Ok(())
}

#[test]
fn at_capacity_the_oldest_unheld_slot_is_evicted() -> Result<(), Box<dyn Error>> {
    // Given: a full scratchpad of unheld slots.
    let mut wm = WorkingMemory::new(2);
    wm.update("a", 1);
    wm.update("b", 2);

    // When: a third slot is written.
    assert!(wm.update("c", 3));

    // Then: the oldest unheld slot is evicted, the rest retained.
    assert_eq!(wm.len(), 2);
    assert_eq!(wm.get("a"), None);
    assert_eq!(wm.get("b"), Some(&2));
    assert_eq!(wm.get("c"), Some(&3));
    Ok(())
}

#[test]
fn held_slots_are_protected_from_eviction() -> Result<(), Box<dyn Error>> {
    // Given: a full scratchpad whose oldest slot is held.
    let mut wm = WorkingMemory::new(2);
    wm.update("goal", "stay active");
    wm.update("scratch", "transient");
    assert!(wm.hold("goal"));

    // When: a new slot forces an eviction.
    assert!(wm.update("scratch2", "newer transient"));

    // Then: the held goal survives; the unheld scratch is evicted instead.
    assert!(wm.is_held("goal"));
    assert_eq!(wm.get("goal"), Some(&"stay active"));
    assert_eq!(wm.get("scratch"), None);
    assert_eq!(wm.get("scratch2"), Some(&"newer transient"));
    Ok(())
}

#[test]
fn a_fully_held_scratchpad_refuses_new_slots() -> Result<(), Box<dyn Error>> {
    // Given: a full scratchpad with every slot held.
    let mut wm = WorkingMemory::new(2);
    wm.update("a", 1);
    wm.update("b", 2);
    assert!(wm.hold("a"));
    assert!(wm.hold("b"));

    // When/Then: a new key cannot be admitted while context is fully protected,
    // until a slot is released.
    assert!(!wm.update("c", 3));
    assert_eq!(wm.get("c"), None);
    assert!(wm.release("a"));
    assert!(wm.update("c", 3));
    assert_eq!(wm.get("c"), Some(&3));
    Ok(())
}
