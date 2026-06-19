use std::error::Error;
use std::path::PathBuf;

use axon::exec::{Decision, Executor, Plan, Step};
use axon::memory::EpisodicStore;
use axon::modulate::Modulators;
use axon::predict::{Expected, Outcome, Prediction, Verifier};
use axon::tools::{FsRead, Tool};
use axon::workspace::Workspace;

fn main() -> Result<(), Box<dyn Error>> {
    let mut read = FsRead::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let manifest = read.call("Cargo.toml".to_owned())?;
    let prediction = Prediction::new("read manifest", Expected::Contains("axon".to_owned()));
    let plan = Plan::new([Step::new("read manifest", prediction)]);
    let mut executor = Executor::new(
        EpisodicStore::new(),
        Verifier,
        Modulators::baseline(),
        Workspace::new(8)?,
    );

    let decision = executor.observe_step(&plan, 0, Outcome::new(manifest))?;
    assert_eq!(decision, Decision::Continue);
    assert_eq!(executor.memory().episodes().len(), 1);
    assert_eq!(executor.workspace().broadcasts().len(), 1);
    Ok(())
}
