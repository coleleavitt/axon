use std::error::Error;

use axon_core::{
    Allow,
    BreakerState,
    CircuitBreaker,
    FnModule,
    Gate,
    InputId,
    ModuleId,
    ModuleOutput,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

#[test]
fn breaker_opens_after_reaching_its_failure_threshold() -> Result<(), Box<dyn Error>> {
    // Given: a breaker that tolerates one failure before opening.
    let breaker = CircuitBreaker::new(2);
    let probe: Signal<()> = Signal::new(());

    // When/Then: it admits while closed, stays closed below threshold, opens at
    // it, and closes again on reset.
    assert!(breaker.admits(&probe));
    breaker.trip();
    assert!(!breaker.is_open());
    breaker.trip();
    assert!(breaker.is_open());
    assert!(!breaker.admits(&probe));
    breaker.reset();
    assert_eq!(breaker.failures(), 0);
    assert!(breaker.admits(&probe));
    Ok(())
}

#[test]
fn open_breaker_isolates_a_route_in_the_runtime() -> Result<(), Box<dyn Error>> {
    // Given: a worker reachable only through a breaker-gated route.
    let breaker = CircuitBreaker::new(1);
    let input = InputId::new("input")?;
    let worker = ModuleId::new("worker")?;
    let mut runtime = Runtime::default();
    runtime.insert_module(FnModule::new(worker.clone(), |signal: Signal<u8>| {
        Ok(ModuleOutput::stop_signal(signal))
    }))?;
    runtime.add_input_route(input.clone(), worker, Weight::new(10), breaker.clone())?;

    // When: the circuit is closed, the signal reaches the worker and stops.
    let report = runtime.run(input.clone(), Signal::new(1u8))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));

    // Then: once the breaker trips open, the same route no longer admits and the
    // run ends with no route rather than cascading into the failing module.
    breaker.trip();
    let isolated = runtime.run(input, Signal::new(2u8))?;
    assert!(matches!(isolated.status(), RunStatus::NoRoute { .. }));
    Ok(())
}

#[test]
fn health_score_degrades_then_opens() -> Result<(), Box<dyn Error>> {
    // Given: a breaker tolerating four failures.
    let breaker = CircuitBreaker::new(4);
    assert_eq!(breaker.state(), BreakerState::Closed);
    assert!((breaker.health() - 1.0).abs() < f32::EPSILON);

    // When/Then: halfway to threshold it is degraded but still admitting.
    breaker.trip();
    breaker.trip();
    assert_eq!(breaker.state(), BreakerState::Degraded);
    assert!((breaker.health() - 0.5).abs() < f32::EPSILON);

    // And: at threshold it is open with zero health.
    breaker.trip();
    breaker.trip();
    assert_eq!(breaker.state(), BreakerState::Open);
    assert!(breaker.health() < f32::EPSILON);
    Ok(())
}

#[test]
fn half_open_probe_allows_one_trial_then_recovers() -> Result<(), Box<dyn Error>> {
    // Given: an open breaker.
    let breaker = CircuitBreaker::new(1);
    let probe: Signal<()> = Signal::new(());
    breaker.record_failure();
    assert!(breaker.is_open());
    assert!(!breaker.admits(&probe));

    // When: a half-open probe is permitted, exactly one signal gets through.
    breaker.allow_probe();
    assert!(breaker.admits(&probe));
    assert!(!breaker.admits(&probe));

    // Then: recording the probe's success closes the breaker again.
    breaker.record_success();
    assert_eq!(breaker.state(), BreakerState::Closed);
    assert!(breaker.admits(&probe));
    Ok(())
}

#[test]
fn an_open_breaker_degrades_to_a_lower_weight_fallback() -> Result<(), Box<dyn Error>> {
    // Given: a primary route guarded by a breaker, plus a lower-weight fallback.
    let breaker = CircuitBreaker::new(1);
    let mut runtime: Runtime<u32> = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(
        ModuleId::new("primary")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.insert_module(FnModule::new(
        ModuleId::new("fallback")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("primary")?,
        Weight::new(10),
        breaker.clone(),
    )?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("fallback")?,
        Weight::new(1),
        Allow,
    )?;

    // When healthy: the primary (higher weight) wins.
    let healthy = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert_eq!(healthy.steps()[0].edge().to(), &ModuleId::new("primary")?);

    // When the breaker opens: the primary is no longer admitted, so routing
    // degrades gracefully to the reduced-capability fallback instead of failing.
    breaker.record_failure();
    let degraded = runtime.run(InputId::new("in")?, Signal::new(2))?;
    assert_eq!(degraded.steps()[0].edge().to(), &ModuleId::new("fallback")?);
    Ok(())
}
