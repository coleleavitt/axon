//! A real [`Provider`] backed by the OpenAI Chat Completions API.
//!
//! Gated behind the `openai` feature so the seam itself stays dependency-free.
//! The HTTP body shaping and response parsing are pure functions
//! ([`build_request_body`], [`extract_completion`]) so they can be tested
//! offline; only [`OpenAiProvider::complete`] touches the network.

use std::{env, fmt};

use axon_core::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::{Completion, Provider, ProviderError};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-5.5";

/// Calls OpenAI's `POST /chat/completions` endpoint behind the [`Provider`] seam.
///
/// `Debug` is implemented by hand to redact the API key, so logging or
/// panicking with a provider in scope never leaks the secret.
#[derive(Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    /// Build a provider for `model` authenticated with `api_key`, targeting the
    /// public OpenAI API.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
        }
    }

    /// Build a provider from the environment: `OPENAI_API_KEY` (required),
    /// `OPENAI_MODEL` (default `gpt-5.5`), `OPENAI_BASE_URL` (default the public
    /// API; override for Azure/proxies/OpenAI-compatible servers). Empty values
    /// are treated as unset. A non-https override is rejected unless it points at
    /// a loopback host, so the bearer token is never sent in cleartext.
    pub fn from_env() -> Result<Self, ProviderError> {
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| ProviderError::Unavailable {
            reason: "OPENAI_API_KEY is not set".to_owned(),
        })?;
        let model = env::var("OPENAI_MODEL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
        let mut provider = Self::new(api_key, model);
        if let Ok(base_url) = env::var("OPENAI_BASE_URL") {
            let trimmed = base_url.trim();
            if !trimmed.is_empty() {
                provider = provider.with_base_url(trimmed)?;
            }
        }
        Ok(provider)
    }

    /// Point the provider at a different OpenAI-compatible base URL. The scheme
    /// must be `https`, or `http` to a loopback host (e.g. a local model server);
    /// any trailing slash is trimmed so the endpoint join stays well-formed.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let base_url = base_url.into();
        validate_base_url(&base_url)?;
        self.base_url = base_url.trim_end_matches('/').to_owned();
        Ok(self)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl fmt::Debug for OpenAiProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiProvider")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl Provider for OpenAiProvider {
    fn complete(&self, prompt: &str) -> BoxFuture<'_, Result<Completion, ProviderError>> {
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let body = build_request_body(&self.model, prompt);
        Box::pin(async move {
            let response = client
                .post(&url)
                .bearer_auth(&api_key)
                .json(&body)
                .send()
                .await
                .map_err(|error| ProviderError::Unavailable {
                    reason: error.to_string(),
                })?;
            let status = response.status();
            if !status.is_success() {
                let detail = response.text().await.unwrap_or_default();
                return Err(ProviderError::Unavailable {
                    reason: format!("openai returned {status}: {detail}"),
                });
            }
            let parsed: ChatResponse =
                response
                    .json()
                    .await
                    .map_err(|error| ProviderError::Unavailable {
                        reason: error.to_string(),
                    })?;
            extract_completion(parsed)
        })
    }
}

/// `https` always; `http` only to a loopback host (local model servers), so the
/// bearer token is never transmitted in cleartext over a network.
fn validate_base_url(base_url: &str) -> Result<(), ProviderError> {
    let reject = |reason: String| Err(ProviderError::Unavailable { reason });
    if let Some(rest) = base_url.strip_prefix("https://") {
        if rest.is_empty() {
            return reject("OPENAI_BASE_URL host is empty".to_owned());
        }
        Ok(())
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or("");
        if matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]") {
            Ok(())
        } else {
            reject(format!(
                "refusing to send the API key over cleartext http to non-loopback host {host:?}; use https"
            ))
        }
    } else {
        reject(format!(
            "OPENAI_BASE_URL must start with http:// or https://, got {base_url:?}"
        ))
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<RequestMessage>,
}

#[derive(Debug, Serialize)]
struct RequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    /// Null for turns that carry no text (refusals, tool-only replies).
    content: Option<String>,
}

/// Shape a single-user-message chat request. Pure: no network.
fn build_request_body(model: &str, prompt: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_owned(),
        messages: vec![RequestMessage {
            role: "user".to_owned(),
            content: prompt.to_owned(),
        }],
    }
}

/// Pull the first choice's text out of a parsed response, treating a null
/// content as an empty completion rather than a hard failure. Pure: no network.
fn extract_completion(response: ChatResponse) -> Result<Completion, ProviderError> {
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| Completion::new(choice.message.content.unwrap_or_default()))
        .ok_or_else(|| ProviderError::Unavailable {
            reason: "openai response contained no choices".to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn request_body_wraps_the_prompt_as_a_user_message() -> Result<(), Box<dyn Error>> {
        let body = build_request_body("gpt-5.5", "summarize the manifest");
        let json = serde_json::to_value(&body)?;
        assert_eq!(json["model"], "gpt-5.5");
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "summarize the manifest");
        Ok(())
    }

    #[test]
    fn extracts_first_choice_text() -> Result<(), Box<dyn Error>> {
        let response: ChatResponse = serde_json::from_str(
            r#"{"choices":[{"message":{"role":"assistant","content":"list files\nread manifest"}}]}"#,
        )?;
        assert_eq!(
            extract_completion(response)?.text(),
            "list files\nread manifest"
        );
        Ok(())
    }

    #[test]
    fn null_content_yields_an_empty_completion() -> Result<(), Box<dyn Error>> {
        let response: ChatResponse = serde_json::from_str(
            r#"{"choices":[{"message":{"role":"assistant","content":null}}]}"#,
        )?;
        assert_eq!(extract_completion(response)?.text(), "");
        Ok(())
    }

    #[test]
    fn empty_choices_is_an_error() -> Result<(), Box<dyn Error>> {
        let response: ChatResponse = serde_json::from_str(r#"{"choices":[]}"#)?;
        assert!(matches!(
            extract_completion(response),
            Err(ProviderError::Unavailable { .. })
        ));
        Ok(())
    }

    #[test]
    fn with_base_url_trims_trailing_slash() -> Result<(), Box<dyn Error>> {
        let provider = OpenAiProvider::new("k", "m").with_base_url("https://host/v1/")?;
        assert_eq!(provider.base_url(), "https://host/v1");
        Ok(())
    }

    #[test]
    fn rejects_cleartext_http_to_remote_host() {
        let result = OpenAiProvider::new("k", "m").with_base_url("http://api.example.com/v1");
        assert!(result.is_err());
    }

    #[test]
    fn allows_http_to_loopback() -> Result<(), Box<dyn Error>> {
        let provider = OpenAiProvider::new("k", "m").with_base_url("http://localhost:11434/v1")?;
        assert_eq!(provider.base_url(), "http://localhost:11434/v1");
        Ok(())
    }

    #[test]
    fn debug_redacts_the_api_key() {
        let rendered = format!("{:?}", OpenAiProvider::new("super-secret-key", "gpt-5.5"));
        assert!(!rendered.contains("super-secret-key"));
        assert!(rendered.contains("redacted"));
    }
}
