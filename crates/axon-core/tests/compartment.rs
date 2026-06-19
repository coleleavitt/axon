use std::error::Error;

use axon_core::{CompartmentModule, Module, ModuleId, ModuleOutput, Signal};

#[test]
fn a_compartment_module_fires_only_on_branch_coincidence() -> Result<(), Box<dyn Error>> {
    // Given: a node with two dendritic branches that fires only when both agree
    // (a coincidence threshold — a nonlinear leaf, not a flat pass-through).
    let mut module = CompartmentModule::<String>::new(ModuleId::new("dendrite")?, 2)
        .with_branch(|signal: &Signal<String>| signal.payload().contains('a'))
        .with_branch(|signal: &Signal<String>| signal.payload().contains('b'));
    assert_eq!(module.branch_count(), 2);

    // When/Then: both branches fire -> the signal passes on.
    assert!(matches!(
        module.handle(Signal::new("ab".to_owned()))?,
        ModuleOutput::Emit(_)
    ));

    // Only one branch fires -> below threshold -> dropped.
    assert!(matches!(
        module.handle(Signal::new("a only".to_owned()))?,
        ModuleOutput::Drop
    ));

    // Neither fires -> dropped.
    assert!(matches!(
        module.handle(Signal::new("zzz".to_owned()))?,
        ModuleOutput::Drop
    ));
    Ok(())
}
