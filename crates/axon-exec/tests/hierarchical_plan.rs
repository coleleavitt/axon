use std::error::Error;

use axon_exec::{Plan, Step};
use axon_predict::{Expected, Prediction};

fn step(action: &'static str) -> Step {
    Step::new(action, Prediction::new(action, Expected::Anything))
}

#[test]
fn plans_compose_hierarchically_into_one_sequence() -> Result<(), Box<dyn Error>> {
    // Given: two sub-goal plans.
    let setup = Plan::new([step("clone repo")]);
    let build = Plan::new([step("cargo build"), step("cargo test")]);

    // When: they are composed into a single hierarchical plan.
    let full = Plan::compose([setup, build]);

    // Then: the sub-goals expand in order into one flat step sequence.
    assert_eq!(full.len(), 3);
    assert_eq!(full.step(0).map(Step::action), Some("clone repo"));
    assert_eq!(full.step(2).map(Step::action), Some("cargo test"));

    // And: `then` appends a further sub-goal plan.
    let extended = full.then(Plan::new([step("deploy")]));
    assert_eq!(extended.len(), 4);
    assert_eq!(extended.step(3).map(Step::action), Some("deploy"));
    Ok(())
}
