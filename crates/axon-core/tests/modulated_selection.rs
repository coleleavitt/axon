use std::error::Error;

use axon_core::{
    Allow,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

/// Two competing routes from one input: `a` (weight 5) beats `b` (4) under
/// argmax. Both modules stop, so a run traverses exactly the winning edge.
fn two_route_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
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

/// Run `count` times and return the winning module id of each run.
fn winners(runtime: &mut Runtime<u32>, count: usize) -> Result<Vec<String>, Box<dyn Error>> {
    let mut out = Vec::with_capacity(count);
    for value in 0..count {
        let report = runtime.run(InputId::new("in")?, Signal::new(value as u32))?;
        out.push(report.steps()[0].edge().to().as_str().to_owned());
    }
    Ok(out)
}

#[test]
fn zero_exploration_is_deterministic_argmax() -> Result<(), Box<dyn Error>> {
    // Given: the default runtime (exploration 0).
    let mut runtime = two_route_runtime()?;

    // When/Then: every run picks the argmax winner `a`; the knob is off.
    let winners = winners(&mut runtime, 20)?;
    assert!(winners.iter().all(|winner| winner == "a"));
    Ok(())
}

#[test]
fn exploration_lets_lower_weight_routes_win() -> Result<(), Box<dyn Error>> {
    // Given: a high exploration temperature over close weights (5 vs 4).
    let mut runtime = two_route_runtime()?.with_exploration(10.0).with_seed(42);

    // When: many runs are sampled.
    let winners = winners(&mut runtime, 200)?;

    // Then: both routes win at least once — `exploration` is genuinely
    // load-bearing, not a no-op that always returns the argmax.
    assert!(winners.iter().any(|winner| winner == "a"));
    assert!(winners.iter().any(|winner| winner == "b"));
    Ok(())
}

#[test]
fn same_seed_reproduces_the_same_trajectory_sequence() -> Result<(), Box<dyn Error>> {
    // Given: two independent runtimes with identical seed and exploration.
    let mut left = two_route_runtime()?.with_exploration(5.0).with_seed(7);
    let mut right = two_route_runtime()?.with_exploration(5.0).with_seed(7);

    // When/Then: the stochastic winner sequences are bit-for-bit identical —
    // seeded RNG keeps exploratory runs replayable.
    assert_eq!(winners(&mut left, 64)?, winners(&mut right, 64)?);
    assert_eq!(left.seed(), 7);
    Ok(())
}

#[test]
fn different_seeds_can_diverge() -> Result<(), Box<dyn Error>> {
    // Given: identical runtimes differing only by seed.
    let mut left = two_route_runtime()?.with_exploration(5.0).with_seed(1);
    let mut right = two_route_runtime()?.with_exploration(5.0).with_seed(999);

    // When/Then: the seed actually selects the random stream, so the two
    // sequences are not forced to coincide.
    assert_ne!(winners(&mut left, 64)?, winners(&mut right, 64)?);
    Ok(())
}
