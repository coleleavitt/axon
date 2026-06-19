use std::error::Error;

use axon_workspace::{Broadcast, Goal, Workspace};

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
