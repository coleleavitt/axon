use std::collections::HashMap;
use std::error::Error;

use axon_provider::{MockProvider, Provider, ProviderError};

#[tokio::test]
async fn scripted_provider_answers_every_prompt_the_same() -> Result<(), Box<dyn Error>> {
    // Given: a provider scripted with a fixed completion.
    let provider = MockProvider::scripted("list files\nread manifest");

    // When: it is asked anything.
    let completion = provider.complete("whatever the prompt is").await?;

    // Then: it returns the scripted text deterministically.
    assert_eq!(completion.text(), "list files\nread manifest");
    Ok(())
}

#[tokio::test]
async fn keyed_provider_errors_on_an_unscripted_prompt() -> Result<(), Box<dyn Error>> {
    // Given: a provider that only knows one prompt.
    let mut responses = HashMap::new();
    responses.insert("ping".to_owned(), "pong".to_owned());
    let provider = MockProvider::keyed(responses);

    // When/Then: the known prompt answers, the unknown one reports no response.
    assert_eq!(provider.complete("ping").await?.text(), "pong");
    match provider.complete("unknown").await {
        Err(ProviderError::NoResponse { prompt }) => assert_eq!(prompt, "unknown"),
        other => panic!("expected NoResponse, got {other:?}"),
    }
    Ok(())
}
