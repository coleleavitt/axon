use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};
use axon_exec::{AgentSignal, Executive, Plan, Planner, RoutedTool, SkillLibrary, Step, wire_loop};
use axon_memory::EpisodicStore;
use axon_modulate::Modulators;
use axon_predict::{Expected, Prediction, Verifier};
use axon_workspace::Workspace;

fn step(action: &'static str) -> Step {
    Step::new(
        action,
        Prediction::new(action, Expected::Contains("ok".to_owned())),
    )
}

#[test]
fn a_successful_plan_is_promoted_and_recalled_as_a_runnable_skill() -> Result<(), Box<dyn Error>> {
    // Given: a known-good plan for a goal, promoted into the skill library.
    let goal = "tidy the repo";
    let plan = Plan::new([step("format"), step("test")]);
    let mut library = SkillLibrary::new();
    assert!(library.is_empty());
    library.learn(goal, &plan);
    assert_eq!(library.len(), 1);

    // When: a new session faces the same goal — recall the skill, don't re-plan.
    let Some(recalled) = library.recall(goal) else {
        panic!("expected a recalled skill");
    };

    // Then: the recalled plan preserves the action sequence.
    assert_eq!(recalled.len(), 2);
    assert_eq!(recalled.step(0).map(Step::action), Some("format"));
    assert_eq!(recalled.step(1).map(Step::action), Some("test"));

    // A fuzzy goal cue still finds it; an unknown goal does not.
    assert!(library.recall_similar("please tidy repo now").is_some());
    assert!(library.recall("compile the kernel").is_none());

    // And the recalled skill actually runs to completion through the routed loop.
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));
    let entry = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), recalled))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), |_action: &str| {
        Ok::<_, ModuleError>("ok".to_owned())
    }))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &entry, &planner, &tool, &executive)?;

    let report = runtime.run(entry, Signal::new(AgentSignal::Goal))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    // Both recalled steps executed.
    assert_eq!(memory.borrow().episodes().len(), 2);
    Ok(())
}
