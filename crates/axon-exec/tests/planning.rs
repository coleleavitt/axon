use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};
use axon_exec::{AgentSignal, Executive, Planner, RoutedTool, propose_plan, wire_loop};
use axon_memory::EpisodicStore;
use axon_modulate::Modulators;
use axon_predict::{Expected, Verifier};
use axon_provider::MockProvider;
use axon_workspace::Workspace;

#[tokio::test]
async fn provider_plan_drives_the_routed_core_loop() -> Result<(), Box<dyn Error>> {
    // Given: a provider that proposes two actions, the second with an
    // expectation attached via the `=>` convention.
    let provider = MockProvider::scripted("list files\nread manifest => axon");

    // When: the goal is decomposed into a typed plan.
    let plan = propose_plan(&provider, "ship axon").await?;

    // Then: the model output became typed steps with parsed predictions.
    assert_eq!(plan.steps().len(), 2);
    assert_eq!(plan.steps()[0].action(), "list files");
    assert_eq!(plan.steps()[0].prediction().expected(), &Expected::Anything);
    assert_eq!(plan.steps()[1].action(), "read manifest");
    assert_eq!(
        plan.steps()[1].prediction().expected(),
        &Expected::Contains("axon".to_owned())
    );

    // And: that plan runs through the routing core unchanged.
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));
    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(
        tool.clone(),
        |action: &str| -> Result<String, ModuleError> {
            match action {
                "list files" => Ok("Cargo.toml".to_owned()),
                "read manifest" => Ok("name = axon".to_owned()),
                other => Err(ModuleError::new(format!("unknown action: {other}"))),
            }
        },
    ))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    assert_eq!(memory.borrow().episodes().len(), 2);
    Ok(())
}
