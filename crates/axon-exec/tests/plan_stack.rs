use std::error::Error;

use axon_exec::{Plan, PlanStack, Step};
use axon_predict::{Expected, Prediction};

fn step(action: &'static str) -> Step {
    Step::new(action, Prediction::new(action, Expected::Anything))
}

#[test]
fn sub_goals_expand_lazily_and_only_when_their_precondition_holds() -> Result<(), Box<dyn Error>> {
    // Given: a root plan of two sub-goals: "setup" then "build".
    let mut stack = PlanStack::new(Plan::new([step("setup"), step("build")]));
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.current().map(Step::action), Some("setup"));

    // When: "setup" is treated as a leaf and completed.
    stack.advance();
    assert_eq!(stack.current().map(Step::action), Some("build"));

    // And: "build" is a sub-goal — consume the marker and lazily expand its
    // decomposition, gated on a precondition that holds.
    stack.advance();
    assert!(stack.expand_if(Plan::new([step("compile"), step("test")]), true));
    assert_eq!(stack.depth(), 2);

    // Then: the child frame runs to completion before control returns to the root.
    assert_eq!(stack.current().map(Step::action), Some("compile"));
    stack.advance();
    assert_eq!(stack.current().map(Step::action), Some("test"));
    stack.advance();

    // The child is exhausted, so it is popped back to the (also exhausted) root.
    assert!(stack.is_done());
    assert_eq!(stack.depth(), 1);
    Ok(())
}

#[test]
fn a_failed_precondition_does_not_expand_the_sub_goal() -> Result<(), Box<dyn Error>> {
    // Given: a single-step root plan.
    let mut stack = PlanStack::new(Plan::new([step("maybe")]));

    // When: expansion is attempted with a precondition that does not hold.
    let expanded = stack.expand_if(Plan::new([step("never")]), false);

    // Then: no child frame is released; the root continues unchanged.
    assert!(!expanded);
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.current().map(Step::action), Some("maybe"));
    Ok(())
}
