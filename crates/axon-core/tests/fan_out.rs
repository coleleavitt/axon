use std::error::Error;

use axon_core::{
    Allow,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Route,
    RoutingTable,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

#[test]
fn select_all_returns_top_k_by_weight_deterministically() -> Result<(), Box<dyn Error>> {
    // Given: three admitted routes from one input at different weights.
    let mut table: RoutingTable<()> = RoutingTable::new();
    let input = EndpointId::from(InputId::new("in")?);
    table.push(Route::open(
        input.clone(),
        ModuleId::new("low")?,
        Weight::new(1),
    ));
    table.push(Route::open(
        input.clone(),
        ModuleId::new("high")?,
        Weight::new(9),
    ));
    table.push(Route::open(
        input.clone(),
        ModuleId::new("mid")?,
        Weight::new(5),
    ));
    let signal = Signal::new(());

    // When: the top two are recruited.
    let top_two = table.select_all(&input, &signal, 2);

    // Then: they are the two heaviest, in descending weight order.
    assert_eq!(top_two.len(), 2);
    assert_eq!(top_two[0].to(), &ModuleId::new("high")?);
    assert_eq!(top_two[1].to(), &ModuleId::new("mid")?);

    // And: a limit beyond the available count returns all, still weight-ordered.
    let all = table.select_all(&input, &signal, 10);
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].to(), &ModuleId::new("low")?);
    Ok(())
}

#[test]
fn runtime_recruits_several_modules_for_one_signal() -> Result<(), Box<dyn Error>> {
    // Given: a runtime where one input feeds three modules at different weights.
    let mut runtime = Runtime::new(StepLimit::default());
    for name in ["writer", "interrupt", "logger"] {
        runtime.insert_module(FnModule::new(
            ModuleId::new(name)?,
            |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
        ))?;
    }
    let alert = InputId::new("alert")?;
    runtime.add_input_route(
        alert.clone(),
        ModuleId::new("writer")?,
        Weight::new(8),
        Allow,
    )?;
    runtime.add_input_route(
        alert.clone(),
        ModuleId::new("interrupt")?,
        Weight::new(9),
        Allow,
    )?;
    runtime.add_input_route(
        alert.clone(),
        ModuleId::new("logger")?,
        Weight::new(1),
        Allow,
    )?;

    // When: the runtime recruits the top two targets for one alert signal.
    let recruited = runtime.recruit(&EndpointId::from(alert), &Signal::new(1), 2);

    // Then: one signal drives the two highest-weight modules at once.
    assert_eq!(
        recruited,
        vec![ModuleId::new("interrupt")?, ModuleId::new("writer")?]
    );
    Ok(())
}
