use std::error::Error;

use axon_core::{Credit, Plasticity, ProportionalPlasticity};
use axon_tools::{ToolReport, ToolStatus};

#[test]
fn tool_outcomes_feed_plasticity() -> Result<(), Box<dyn Error>> {
    // Given: a successful and a failed tool report.
    let success = ToolReport::success("wrote 3 files").with_cost(3);
    let failure = ToolReport::failure("error: command timed out");

    // Then: status and cost are captured structurally, not flattened to a string.
    assert!(success.is_success());
    assert_eq!(success.cost(), 3);
    assert_eq!(failure.status(), ToolStatus::Failure);

    // And: the structured outcome scales reinforcement of the route that produced
    // it — a successful call strengthens the path, a failed one weakens it.
    let policy = ProportionalPlasticity::default();
    let delta = |error| {
        policy.delta(Credit {
            error,
            eligibility: 1.0,
            learning_rate: 1.0,
        })
    };
    assert!(delta(success.graded_error()) > 0);
    assert!(delta(failure.graded_error()) < 0);
    Ok(())
}
