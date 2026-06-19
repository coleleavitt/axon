use std::error::Error;

use axon_core::{
    CircuitBreaker,
    FnModule,
    Gate,
    InputId,
    ModuleId,
    ModuleOutput,
    RunStatus,
    Runtime,
    Signal,
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
