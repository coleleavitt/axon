# Axon

Axon is a small Rust SDK core for agentic systems. The core only owns the signal substrate:

- typed `Signal<P>` payloads
- explicit `Route<P>` edges
- default-deny routing through `Gate<P>` implementations
- an execution `Runtime<P>` that drives registered modules until stop, drop, no-route, or step limit
- `FnModule` adapters for small function-backed tool modules

It intentionally does not include LLM providers, memory, tools, or UI. Those belong in modules that plug into the core.

```rust
use axon::{Allow, FnModule, InputId, ModuleError, ModuleId, ModuleOutput, Runtime, Signal, Weight};

#[derive(Debug)]
enum Payload {
    Goal,
    Observation,
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let input = InputId::new("input")?;
let worker = ModuleId::new("worker")?;
let mut runtime = Runtime::default();

runtime.insert_module(FnModule::new(worker.clone(), |_signal: Signal<Payload>| {
    Ok(ModuleOutput::stop(Payload::Observation))
}))?;
runtime.add_input_route(input.clone(), worker, Weight::new(10), Allow)?;

let report = runtime.run(input, Signal::new(Payload::Goal))?;
assert_eq!(report.steps().len(), 1);
# Ok(())
# }
```

Run the library surface example with:

```bash
cargo run --example basic_loop
```

The neuroscience research behind the naming and boundaries lives in `docs/research/`.
