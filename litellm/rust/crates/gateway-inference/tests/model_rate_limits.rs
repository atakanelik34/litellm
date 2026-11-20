mod support;

use rstest::rstest;
use serde_json::{Value, json};
use wiremock::MockServer;

fn prompt() -> Value {
    json!({
        "model": "public/model",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": "hi"}],
    })
}

/// Python's proxy answers this refusal with the same message and no `retry-after`: only
/// `llm_provider-retry-after: 60` on chat, and nothing on `/v1/messages`.
#[rstest]
#[case::messages("/v1/messages")]
#[case::chat_completions("/v1/chat/completions")]
#[tokio::test]
async fn a_deployment_past_its_rpm_answers_429_with_retry_after_and_no_provider_call(
    #[case] path: &str,
) {
    let upstream = MockServer::start().await;
    let app = support::app_with_router_settings(
        "anthropic/test-model",
        &upstream.uri(),
        json!({"rpm": 0}),
        json!({"optional_pre_call_checks": ["enforce_model_rate_limits"]}),
    );

    let refused = support::post(app, path, prompt()).await;

    let status = refused.status().as_u16();
    let retry_after = refused.headers().get("retry-after").cloned();
    let body = support::json(refused).await;
    assert_eq!(
        (
            status,
            retry_after.as_ref().and_then(|value| value.to_str().ok()),
            body.pointer("/error/message").and_then(Value::as_str),
            upstream.received_requests().await.unwrap().len(),
        ),
        (
            429,
            Some("60"),
            Some("litellm.RateLimitError: Model rate limit exceeded. RPM limit=0, current usage=1"),
            0,
        )
    );
}
