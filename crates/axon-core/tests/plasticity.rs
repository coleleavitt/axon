use std::error::Error;

use axon_core::{
    Allow,
    Credit,
    EdgeId,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Plasticity,
    ProportionalPlasticity,
    Reinforcement,
    RunEvent,
    RunStatus,
    Runtime,
    Signal,
    Weight,
};

/// Two competing routes from one input: `a` (weight 5) initially beats `b` (4).
/// Both modules simply stop, so a run traverses exactly the winning edge.
fn two_route_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::default();
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.insert_module(FnModule::new(ModuleId::new("b")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(5),
        Allow,
    )?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("b")?,
        Weight::new(4),
        Allow,
    )?;
    Ok(runtime)
}

/// A two-hop chain `in -> a -> b`: `a` emits onward, `b` stops.
fn chain_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::default();
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::emit(*signal.payload()))
    }))?;
    runtime.insert_module(FnModule::new(ModuleId::new("b")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(1),
        Allow,
    )?;
    runtime.add_module_route(
        ModuleId::new("a")?,
        ModuleId::new("b")?,
        Weight::new(1),
        Allow,
    )?;
    Ok(runtime)
}

fn input_edge(to: &'static str) -> Result<EdgeId, Box<dyn Error>> {
    Ok(EdgeId::new(
        EndpointId::from(InputId::new("in")?),
        ModuleId::new(to)?,
    ))
}

fn learned_of(snapshot: &[(EdgeId, i16)], edge: &EdgeId) -> Option<i16> {
    snapshot
        .iter()
        .find(|entry| &entry.0 == edge)
        .map(|entry| entry.1)
}

#[test]
fn proportional_plasticity_signs_credit_by_error() -> Result<(), Box<dyn Error>> {
    let policy = ProportionalPlasticity::new(10);

    // A perfect outcome (error 0) strengthens, a total miss (error 1) weakens,
    // and the midpoint is neutral.
    assert!(
        policy.delta(Credit {
            error: 0.0,
            eligibility: 1.0,
            learning_rate: 1.0,
        }) > 0
    );
    assert!(
        policy.delta(Credit {
            error: 1.0,
            eligibility: 1.0,
            learning_rate: 1.0,
        }) < 0
    );
    assert_eq!(
        policy.delta(Credit {
            error: 0.5,
            eligibility: 1.0,
            learning_rate: 1.0,
        }),
        0
    );
    Ok(())
}

#[test]
fn a_failing_route_is_demoted_below_its_competitor() -> Result<(), Box<dyn Error>> {
    // Given: `a` wins the first run.
    let mut runtime = two_route_runtime()?;
    let first = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(first.status(), RunStatus::Stopped(_)));
    assert_eq!(first.steps()[0].edge().to(), &ModuleId::new("a")?);

    // When: `a` is blamed for a total failure (error 1.0).
    let plasticity = ProportionalPlasticity::default();
    let mut events = Vec::new();
    runtime.reinforce(
        &plasticity,
        first.steps(),
        Reinforcement::new(1.0, 1.0, 0.9),
        &mut |event| events.push(event.clone()),
    );

    // Then: reinforcement reports demoting edge `a` by the full step...
    assert_eq!(
        events,
        vec![RunEvent::Reinforced {
            edge: input_edge("a")?,
            delta: -10,
        }]
    );

    // ...and the next run now routes through `b`, the surviving competitor: the
    // agent learned from a bad outcome and changed which path wins.
    let second = runtime.run(InputId::new("in")?, Signal::new(2))?;
    assert_eq!(second.steps()[0].edge().to(), &ModuleId::new("b")?);
    Ok(())
}

#[test]
fn a_successful_route_is_strengthened() -> Result<(), Box<dyn Error>> {
    let mut runtime = two_route_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;

    let plasticity = ProportionalPlasticity::default();
    runtime.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(0.0, 1.0, 0.9),
        &mut |_| {},
    );

    // A perfect outcome strengthens the traversed edge by the full step.
    assert_eq!(
        learned_of(&runtime.learned_weights(), &input_edge("a")?),
        Some(10)
    );
    Ok(())
}

#[test]
fn credit_decays_across_multi_hop_trajectories() -> Result<(), Box<dyn Error>> {
    // Given: a two-hop run `in -> a -> b` that succeeded.
    let mut runtime = chain_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert_eq!(report.steps().len(), 2);

    // When: a successful outcome is credited back with eligibility decay 0.5.
    let plasticity = ProportionalPlasticity::default();
    let mut events = Vec::new();
    runtime.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(0.0, 1.0, 0.5),
        &mut |event| events.push(event.clone()),
    );

    // Then: the edge nearer the outcome (`a -> b`) is reinforced more strongly
    // than the earlier edge (`in -> a`) — graded temporal credit assignment, not
    // a flat "reinforce only the last edge".
    let learned = runtime.learned_weights();
    let a_b = EdgeId::new(EndpointId::from(ModuleId::new("a")?), ModuleId::new("b")?);
    assert_eq!(learned_of(&learned, &a_b), Some(10));
    assert_eq!(learned_of(&learned, &input_edge("a")?), Some(5));
    assert_eq!(events.len(), 2);
    Ok(())
}

#[test]
fn homeostatic_scaling_caps_total_learned_magnitude() -> Result<(), Box<dyn Error>> {
    // Given: a two-hop run that reinforces both traversed edges (in->a, a->b).
    let mut runtime = chain_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    let plasticity = ProportionalPlasticity::default();
    runtime.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(0.0, 1.0, 0.9),
        &mut |_| {},
    );
    let before = runtime.learned_magnitude();
    assert!(before > 10);

    // When: homeostatic scaling caps the total magnitude below what was learned.
    let a_b = EdgeId::new(EndpointId::from(ModuleId::new("a")?), ModuleId::new("b")?);
    let in_a = input_edge("a")?;
    runtime.homeostatic_scale(10);

    // Then: the total is brought within budget, weights shrink, and the relative
    // order is preserved (the more-credited edge stays at least as strong).
    assert!(runtime.learned_magnitude() <= 10);
    let learned = runtime.learned_weights();
    let a_b_w = learned_of(&learned, &a_b).unwrap_or(0);
    let in_a_w = learned_of(&learned, &in_a).unwrap_or(0);
    assert!(a_b_w >= in_a_w);
    assert!(a_b_w > 0);

    // And: scaling with a generous budget is a no-op.
    runtime.homeostatic_scale(1_000);
    assert!(runtime.learned_magnitude() <= 10);
    Ok(())
}

#[test]
fn decay_pulls_learning_back_toward_the_prior() -> Result<(), Box<dyn Error>> {
    let mut runtime = two_route_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    let plasticity = ProportionalPlasticity::default();
    runtime.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(0.0, 1.0, 0.9),
        &mut |_| {},
    );
    assert_eq!(
        learned_of(&runtime.learned_weights(), &input_edge("a")?),
        Some(10)
    );

    // Decaying by half pulls the learned component halfway back to the prior.
    runtime.decay(0.5);
    assert_eq!(
        learned_of(&runtime.learned_weights(), &input_edge("a")?),
        Some(5)
    );
    Ok(())
}

#[test]
fn learned_weights_snapshot_restores_routing_behavior() -> Result<(), Box<dyn Error>> {
    // Train one runtime until a failing `a` is demoted below `b`.
    let mut trained = two_route_runtime()?;
    let report = trained.run(InputId::new("in")?, Signal::new(1))?;
    let plasticity = ProportionalPlasticity::default();
    trained.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(1.0, 1.0, 0.9),
        &mut |_| {},
    );
    let snapshot = trained.learned_weights();

    // A fresh runtime restoring that snapshot inherits the learned routing.
    let mut restored = two_route_runtime()?;
    restored.restore_learned(&snapshot);
    let report = restored.run(InputId::new("in")?, Signal::new(2))?;
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("b")?);
    Ok(())
}

#[cfg(feature = "serde")]
#[test]
fn learned_weights_survive_serde_round_trip() -> Result<(), Box<dyn Error>> {
    let mut trained = two_route_runtime()?;
    let report = trained.run(InputId::new("in")?, Signal::new(1))?;
    let plasticity = ProportionalPlasticity::default();
    trained.reinforce(
        &plasticity,
        report.steps(),
        Reinforcement::new(1.0, 1.0, 0.9),
        &mut |_| {},
    );
    let snapshot = trained.learned_weights();

    // The snapshot serializes and deserializes unchanged...
    let json = serde_json::to_string(&snapshot)?;
    let round_tripped: Vec<(EdgeId, i16)> = serde_json::from_str(&json)?;
    assert_eq!(snapshot, round_tripped);

    // ...and restoring it after a restart reproduces the learned routing.
    let mut restored = two_route_runtime()?;
    restored.restore_learned(&round_tripped);
    let report = restored.run(InputId::new("in")?, Signal::new(2))?;
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("b")?);
    Ok(())
}
