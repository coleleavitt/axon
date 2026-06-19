//! The provider seam: the single typed boundary an LLM call lives behind.
//!
//! The routing core makes no model calls — that is the architectural rule that
//! keeps the substrate deterministic. Model access enters the system only
//! through a [`Provider`], which a planner or tool module holds and awaits. A
//! [`MockProvider`] gives deterministic, offline completions for tests and
//! examples, so the rest of the SDK never has to reach the network to be
//! exercised.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use axon_core::BoxFuture;

/// A model completion: the text a provider produced for a prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    text: String,
}

impl Completion {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }
}

/// The async boundary an LLM call lives behind. Implementors await the model;
/// callers depend only on this trait, never on a concrete vendor client.
pub trait Provider {
    fn complete(&self, prompt: &str) -> BoxFuture<'_, Result<Completion, ProviderError>>;
}

/// A deterministic, offline [`Provider`] for tests and examples.
#[derive(Debug, Clone)]
pub struct MockProvider {
    script: Script,
}

#[derive(Debug, Clone)]
enum Script {
    Always(String),
    Keyed(HashMap<String, String>),
}

impl MockProvider {
    /// Always answer with the same scripted completion, whatever the prompt.
    pub fn scripted(text: impl Into<String>) -> Self {
        Self {
            script: Script::Always(text.into()),
        }
    }

    /// Answer per exact prompt; an unknown prompt yields [`ProviderError::NoResponse`].
    pub fn keyed(responses: HashMap<String, String>) -> Self {
        Self {
            script: Script::Keyed(responses),
        }
    }

    fn answer(&self, prompt: &str) -> Result<Completion, ProviderError> {
        match &self.script {
            Script::Always(text) => Ok(Completion::new(text.clone())),
            Script::Keyed(responses) => {
                responses.get(prompt).map(Completion::new).ok_or_else(|| {
                    ProviderError::NoResponse {
                        prompt: prompt.to_owned(),
                    }
                })
            }
        }
    }
}

impl Provider for MockProvider {
    fn complete(&self, prompt: &str) -> BoxFuture<'_, Result<Completion, ProviderError>> {
        let answer = self.answer(prompt);
        Box::pin(async move { answer })
    }
}

/// Why a provider could not produce a completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// The provider was unreachable or refused the request.
    Unavailable { reason: String },
    /// The provider had no scripted answer for this prompt (mock only).
    NoResponse { prompt: String },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { reason } => write!(formatter, "provider unavailable: {reason}"),
            Self::NoResponse { prompt } => {
                write!(formatter, "provider had no response for prompt: {prompt:?}")
            }
        }
    }
}

impl Error for ProviderError {}
