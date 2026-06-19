use std::error::Error;

use axon_workspace::{Broadcast, BroadcastKind, Goal, Workspace};

#[test]
fn workspace_keeps_bounded_recent_broadcasts() -> Result<(), Box<dyn Error>> {
    // Given: a workspace with capacity two and an active goal.
    let mut workspace = Workspace::new(2)?.with_goal(Goal::new("ship sdk layers")?);

    // When: three observations are broadcast.
    workspace.broadcast(Broadcast::observation("first"));
    workspace.broadcast(Broadcast::observation("second"));
    workspace.broadcast(Broadcast::observation("third"));

    // Then: only the bounded conscious window remains.
    assert_eq!(workspace.goal().map(Goal::as_str), Some("ship sdk layers"));
    assert_eq!(workspace.broadcasts().len(), 2);
    assert_eq!(workspace.broadcasts()[0].text(), "second");
    assert_eq!(workspace.broadcasts()[1].text(), "third");
    Ok(())
}

#[test]
fn workspace_rejects_zero_capacity() -> Result<(), Box<dyn Error>> {
    // Given: a zero capacity request.
    let created = Workspace::new(0);

    // When/Then: the workspace refuses an impossible bounded window.
    assert!(created.is_err());
    Ok(())
}

#[test]
fn routine_observations_cannot_evict_urgent_alerts() -> Result<(), Box<dyn Error>> {
    // Given: a workspace fully occupied by urgent alerts.
    let mut workspace = Workspace::new(2)?;
    assert!(workspace.broadcast(Broadcast::alert("disk full")));
    assert!(workspace.broadcast(Broadcast::alert("auth failed")));

    // When: a routine observation competes against the full alert buffer.
    let admitted = workspace.broadcast(Broadcast::observation("heartbeat"));

    // Then: it loses the competition and is dropped; both alerts remain — a flood
    // of low-salience noise cannot ignite over what matters.
    assert!(!admitted);
    assert_eq!(workspace.broadcasts().len(), 2);
    assert!(
        workspace
            .broadcasts()
            .iter()
            .all(|item| item.kind() == BroadcastKind::Alert)
    );
    Ok(())
}

#[test]
fn a_higher_salience_alert_displaces_the_weakest_observation() -> Result<(), Box<dyn Error>> {
    // Given: a full buffer of routine observations.
    let mut workspace = Workspace::new(2)?;
    assert!(workspace.broadcast(Broadcast::observation("noise one")));
    assert!(workspace.broadcast(Broadcast::observation("noise two")));

    // When: an alert arrives against the full buffer.
    let admitted = workspace.broadcast(Broadcast::alert("critical"));

    // Then: it wins admission, becomes the most salient item, and evicts the
    // oldest weakest observation.
    assert!(admitted);
    assert_eq!(
        workspace.most_salient().map(Broadcast::text),
        Some("critical")
    );
    assert!(
        workspace
            .broadcasts()
            .iter()
            .all(|item| item.text() != "noise one")
    );
    Ok(())
}
