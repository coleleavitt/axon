use std::error::Error;

use axon_exec::{Decision, Executor, Plan, Step};
use axon_memory::{Episode, EpisodicStore, MemoryStore};
use axon_modulate::{Mode, Modulators};
use axon_predict::{Expected, Outcome, Prediction, Verifier};
use axon_workspace::Workspace;

#[test]
fn executor_records_episode_and_proceeds_when_prediction_matches() -> Result<(), Box<dyn Error>> {
    // Given: an executor with memory, verifier, mode knobs, and bounded workspace.
    let memory = EpisodicStore::new();
    let workspace = Workspace::new(4)?;
    let mut executor = Executor::new(memory, Verifier, Modulators::baseline(), workspace);
    let plan = Plan::new([Step::new(
        "read manifest",
        Prediction::new("read manifest", Expected::Contains("axon".to_owned())),
    )]);

    // When: the first step is observed to match its prediction.
    let decision = executor.observe_step(&plan, 0, Outcome::new("package axon"))?;

    // Then: execution proceeds and the episode is stored outside core routing.
    assert_eq!(decision, Decision::Continue);
    assert_eq!(executor.memory().episodes().len(), 1);
    assert!(
        executor.workspace().broadcasts()[0]
            .text()
            .contains("read manifest")
    );
    Ok(())
}

#[test]
fn focused_executor_escalates_prediction_mismatch() -> Result<(), Box<dyn Error>> {
    // Given: a focused executor and an expected outcome that will not appear.
    let memory = EpisodicStore::new();
    let workspace = Workspace::new(4)?;
    let mut executor = Executor::new(
        memory,
        Verifier,
        Modulators::baseline().with_mode(Mode::Focused),
        workspace,
    );
    let plan = Plan::new([Step::new(
        "read manifest",
        Prediction::new("read manifest", Expected::Contains("missing".to_owned())),
    )]);

    // When: observed evidence contradicts the prediction.
    let decision = executor.observe_step(&plan, 0, Outcome::new("package axon"))?;

    // Then: the executor escalates instead of silently continuing.
    match decision {
        Decision::Escalate { reason } => assert!(reason.contains("read manifest")),
        Decision::Continue | Decision::Retry { reason: _ } | Decision::AskUser { question: _ } => {
            panic!("expected escalation")
        }
    }
    executor
        .memory_mut()
        .encode(Episode::new("manual intervention required"));
    Ok(())
}
