use std::fmt;

use axon_core::{Gate, Signal};

use crate::AgentSignal;

/// Appraises the risk of a proposed action in `[0.0, 1.0]` (higher = more
/// dangerous) — the OFC value/risk appraisal a [`RiskGate`] consults.
pub trait RiskAppraiser {
    fn risk(&self, action: &str) -> f32;
}

/// The default appraiser: an action is maximally risky if it mentions any
/// destructive keyword (`rm -rf`, `delete`, `drop`, `--force`, …), else safe. A
/// crude but useful guard for a coding agent.
#[derive(Debug, Clone)]
pub struct KeywordRisk {
    dangerous: Vec<String>,
}

impl KeywordRisk {
    pub fn new<I, S>(keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            dangerous: keywords.into_iter().map(Into::into).collect(),
        }
    }
}

impl Default for KeywordRisk {
    fn default() -> Self {
        Self::new([
            "rm -rf",
            "rm ",
            "delete",
            "drop",
            "--force",
            "force-push",
            "truncate",
            "shutdown",
            "mkfs",
            "format ",
        ])
    }
}

impl RiskAppraiser for KeywordRisk {
    fn risk(&self, action: &str) -> f32 {
        let lowered = action.to_lowercase();
        if self
            .dangerous
            .iter()
            .any(|keyword| lowered.contains(keyword.as_str()))
        {
            1.0
        } else {
            0.0
        }
    }
}

/// A gate that admits an [`AgentSignal::Act`] only when its appraised risk is at
/// or below `tolerance` — proactive response inhibition for high-risk,
/// low-confidence actions (the `rm -rf` / force-push gate). `tolerance` is
/// typically `Modulators::risk_tolerance()`, making that knob load-bearing.
/// Non-Act signals are not its concern and are not admitted (it gates the act
/// hand-off, like `OnAct`).
#[derive(Debug, Clone)]
pub struct RiskGate<A> {
    appraiser: A,
    tolerance: f32,
}

impl<A> RiskGate<A> {
    pub const fn new(appraiser: A, tolerance: f32) -> Self {
        Self {
            appraiser,
            tolerance,
        }
    }
}

impl<A: RiskAppraiser + fmt::Debug> Gate<AgentSignal> for RiskGate<A> {
    fn admits(&self, signal: &Signal<AgentSignal>) -> bool {
        match signal.payload() {
            AgentSignal::Act { action, .. } => self.appraiser.risk(action) <= self.tolerance,
            _ => false,
        }
    }
}
