# Axon

Axon is a small Rust SDK core for agentic systems. The core only owns the signal substrate:

- typed `Signal<P>` payloads
- explicit `Route<P>` edges
- default-deny routing through `Gate<P>` implementations
- an execution `Runtime<P>` that drives registered modules until stop, drop, no-route, or step limit
- `FnModule` adapters for small function-backed tool modules

It intentionally does not include LLM providers, memory, tools, or UI. Those belong in modules that plug into the core.

The repository is now an SDK workspace:

| Crate | Brain analog | Owns |
|---|---|---|
| `axon-core` | axons, thalamus, basal ganglia | typed signals, routes, gates, runtime |
| `axon-tools` | sensorimotor cortex | filesystem, shell, git, typed tool modules |
| `axon-memory` | hippocampus | episodes, deterministic recall, consolidation |
| `axon-predict` | cerebellum | predictions, outcomes, verifier corrections |
| `axon-modulate` | neuromodulators | mode, attention, exploration, risk, learning knobs |
| `axon-workspace` | global workspace | bounded active context and broadcasts |
| `axon-exec` | prefrontal cortex | plan observation, policy decisions, provider-backed planning |
| `axon-provider` | language areas | the async, vendor-agnostic seam model calls live behind |

The root `axon` crate is a facade: it re-exports `axon-core` at the top level and exposes the other crates as modules.

Beyond the routing primitives, the core also offers:

- **A routed agent loop** — the plan→act→observe cycle expressed as `Module`s wired by content-addressed gates (`axon::exec::wire_loop`), so the cognitive layers run *through* the core rather than beside it.
- **An async runtime** — `AsyncRuntime` / `AsyncModule` for modules that do real I/O (tools, providers). It is runtime-agnostic: the library depends on no async executor.
- **Event streaming** — `Runtime::run_observed` streams a `RunEvent` per transition for logging, tracing, or UI.
- **Circuit breaking** — `CircuitBreaker` is a gate that opens after a failure threshold, isolating a route so one module can't cascade.
- **A real provider** — enable the `openai` feature for an OpenAI Chat Completions `Provider`; the default seam ships only the trait and a deterministic `MockProvider`, so it stays dependency-free and offline-testable.
- **Optional persistence** — enable the `serde` feature to snapshot and restore memory, workspace, and neuromodulator state.

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
cargo run --example basic_loop      # the bare routing core
cargo run --example neuro_stack      # layers composed directly in an Executor
cargo run --example integrated_loop  # the same layers driven through the core
cargo run --example llm_planner      # a provider-proposed plan, run with event streaming
```

## Example application

`apps/repo-scout` is a small application that depends on the `axon` SDK the way a downstream consumer would: a provider proposes a plan, `propose_plan` types it, and the routed core executes it with real `FsList`/`FsRead` tools while streaming events.

```bash
cargo run -p repo-scout -- "survey this repository"            # offline, mock provider
OPENAI_API_KEY=... cargo run -p repo-scout --features openai   # real OpenAI planning
```

The neuroscience research behind the naming and boundaries lives in `docs/research/`. A running map of which design principles are implemented versus still missing lives in `docs/research/notes/09_implementation_gap_analysis.md`.
