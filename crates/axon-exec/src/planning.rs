//! Provider-backed planning: turn a natural-language goal into a typed [`Plan`]
//! by asking a [`Provider`] and parsing its completion. This is build-order
//! step five — the planner ("Alfonso") — kept thin: the model proposes actions,
//! the type system pins them down before they ever reach the routing core.

use axon_predict::{Expected, Prediction};
use axon_provider::{Provider, ProviderError};

use crate::{Plan, Step};

/// Separator a model may use to attach an expectation to an action line, e.g.
/// `read manifest => axon`.
const EXPECTATION_DELIMITER: &str = "=>";

/// Build the planning prompt for `goal`. Kept public so callers can inspect or
/// key a mock provider on the exact prompt.
pub fn plan_prompt(goal: &str) -> String {
    format!("Decompose this goal into one concrete action per line: {goal}")
}

/// Ask `provider` to decompose `goal`, parsing its completion into a [`Plan`].
/// Each non-empty line becomes a [`Step`]; a `=>` suffix becomes the step's
/// expected-evidence [`Prediction`], otherwise the step expects anything.
pub async fn propose_plan(provider: &dyn Provider, goal: &str) -> Result<Plan, ProviderError> {
    let completion = provider.complete(&plan_prompt(goal)).await?;
    let steps: Vec<Step> = completion
        .text()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_step)
        .collect();
    Ok(Plan::new(steps))
}

fn parse_step(line: &str) -> Step {
    match line.split_once(EXPECTATION_DELIMITER) {
        Some((action, expected)) => {
            let action = action.trim();
            Step::new(
                action,
                Prediction::new(action, Expected::Contains(expected.trim().to_owned())),
            )
        }
        None => Step::new(line, Prediction::new(line, Expected::Anything)),
    }
}
