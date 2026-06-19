use std::error::Error;

use axon_core::{
    Allow,
    EdgeId,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    ProportionalPlasticity,
    ReplayBuffer,
    Rng,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

fn one_route_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(5),
        Allow,
    )?;
    Ok(runtime)
}

fn learned_of(snapshot: &[(EdgeId, i16)], edge: &EdgeId) -> Option<i16> {
    snapshot
        .iter()
        .find(|entry| &entry.0 == edge)
        .map(|entry| entry.1)
}

#[test]
fn replay_consolidates_learning_offline_without_rerunning_tools() -> Result<(), Box<dyn Error>> {
    // Given: one real run whose trajectory and (clean) outcome are recorded.
    let mut runtime = one_route_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    let mut buffer = ReplayBuffer::new(8);
    buffer.record(report.steps(), 0.0);
    assert_eq!(buffer.len(), 1);

    let edge_a = EdgeId::new(EndpointId::from(InputId::new("in")?), ModuleId::new("a")?);

    // When: the stored experience is replayed five times during an idle pass —
    // no module or tool is run, only memory is reactivated.
    let plasticity = ProportionalPlasticity::default();
    let mut rng = Rng::seeded(1);
    buffer.replay(
        &mut runtime,
        &plasticity,
        1.0,
        0.9,
        5,
        &mut rng,
        &mut |_| {},
    );

    // Then: the traversed edge has been reinforced repeatedly from memory alone
    // (5 replays x the full step), far beyond the single live run.
    let learned = learned_of(&runtime.learned_weights(), &edge_a).unwrap_or(0);
    assert_eq!(learned, 50);
    Ok(())
}

#[test]
fn replay_buffer_is_bounded() -> Result<(), Box<dyn Error>> {
    // Given: a runtime to source a non-empty trajectory.
    let mut runtime = one_route_runtime()?;
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;

    // When: more trajectories are recorded than the capacity.
    let mut buffer = ReplayBuffer::new(2);
    buffer.record(report.steps(), 0.0);
    buffer.record(report.steps(), 0.5);
    buffer.record(report.steps(), 1.0);

    // Then: the buffer keeps only the most recent within capacity.
    assert_eq!(buffer.len(), 2);
    Ok(())
}
