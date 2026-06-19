#![cfg(feature = "serde")]

use std::error::Error;

use axon_workspace::{Broadcast, Goal, Workspace};

#[test]
fn workspace_round_trips_through_json() -> Result<(), Box<dyn Error>> {
    // Given: a workspace holding a goal and a couple of broadcasts.
    let mut workspace = Workspace::new(4)?.with_goal(Goal::new("ship axon")?);
    workspace.broadcast(Broadcast::observation("read manifest"));
    workspace.broadcast(Broadcast::alert("prediction mismatch"));

    // When: it is serialized and restored.
    let json = serde_json::to_string(&workspace)?;
    let restored: Workspace = serde_json::from_str(&json)?;

    // Then: the restored workspace equals the original, capacity and all.
    assert_eq!(restored, workspace);
    assert_eq!(restored.broadcasts().len(), 2);
    Ok(())
}
