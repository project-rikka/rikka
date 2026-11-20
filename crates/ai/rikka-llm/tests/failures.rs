use rikka_llm::wire::{CompletionsDialect, ResponsesDialect};
use rikka_llm::{Error, Stop};
use serde_json::json;

use rikka_test_llm::Server;

use crate::support::answer;

#[tokio::test]
async fn oversized_prompt_is_context_overflow() {
    for body in [
        json!({"error": {"type": "invalid_request_error", "message": "prompt is too long: 210000 tokens"}}),
        json!({"error": {"type": "invalid_request_error", "message": "You passed 1200064 input tokens and requested 1 output tokens. However, the model's context length is only 1048576 tokens, resulting in a maximum input length of 1048575 tokens. Please reduce the length of the input prompt"}}),
        json!({"error": {"type": "invalid_request_error", "message": "", "code": "context_length_exceeded"}}),
    ] {
        let server = Server::failing(400, &body);
        let model = server.messages("claude-opus-5");
        let refused = answer(&model, &model.chat().user("hi").build()).await;
        assert!(matches!(refused, Err(Error::ContextOverflow)), "{body}");
    }

    for body in [
        json!({"error": {"type": "invalid_request_error", "message": "tools.0.name: required"}}),
        json!({"detail": "tools.0.name: required"}),
    ] {
        let server = Server::failing(400, &body);
        let model = server.messages("claude-opus-5");
        let refused = answer(&model, &model.chat().user("hi").build()).await;
        assert!(
            matches!(&refused, Err(Error::Rejected(reason)) if reason.contains("tools.0.name: required")),
            "{refused:?}"
        );
    }
}

#[tokio::test]
async fn exhausted_quota_is_not_rate_limit() {
    let server = Server::failing(
        429,
        &json!({"error": {"type": "insufficient_quota", "message": "", "code": "insufficient_quota"}}),
    );
    let model = server.messages("claude-opus-5");
    let refused = answer(&model, &model.chat().user("hi").build()).await;

    assert!(matches!(refused, Err(Error::QuotaExceeded)), "{refused:?}");
}

#[tokio::test]
async fn account_verification_carries_its_google_page() {
    let refusal = |url: &str| {
        json!({"error": {
            "code": 403,
            "message": "Verify your account to continue.",
            "status": "PERMISSION_DENIED",
            "details": [
                {
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": "VALIDATION_REQUIRED",
                    "domain": "cloudcode-pa.googleapis.com",
                    "metadata": {"validation_url": url}
                },
                {"@type": "type.googleapis.com/google.rpc.Help", "links": [{"url": url}]}
            ]
        }})
    };
    let page = "https://accounts.google.com/v3/signin/continue?plt=token&flowName=GlifWebSignIn";

    for (url, verification) in [
        (page, Some(page)),
        ("https://example.test/continue", None),
        ("http://accounts.google.com/continue", None),
    ] {
        let server = Server::failing(403, &refusal(url));
        let model = server.cloud_code("gemini-3.1-flash-lite");
        let refused = answer(&model, &model.chat().user("hi").build())
            .await
            .expect_err("the account is refused");
        assert_eq!(refused.verification_url(), verification, "{url}");
        if verification.is_none() {
            assert!(matches!(refused, Error::Unauthorized), "{refused:?}");
        }
    }
}

#[tokio::test]
async fn call_cut_by_output_limit_is_dropped() {
    let messages = [
        r#"{"type":"message_start","message":{"id":"msg_1","model":"m","usage":{}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"half"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"write","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\": \"a.rs\", \"text\": \"fn ma"}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":9}}"#,
        r#"{"type":"message_stop"}"#,
    ];
    let completions = [
        r#"{"id":"c","model":"m","choices":[{"index":0,"delta":{"content":"half","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"write","arguments":"{\"path\": \"a.rs\", \"text\": \"fn ma"}}]}}]}"#,
        r#"{"id":"c","choices":[{"index":0,"delta":{},"finish_reason":"length"}]}"#,
        "[DONE]",
    ];
    let responses = [
        r#"{"type":"response.incomplete","response":{"id":"r","model":"m","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"half"}]},{"type":"function_call","call_id":"call_1","name":"write","arguments":"{\"path\": \"a.rs\", \"text\": \"fn ma"}]}}"#,
    ];

    let cut = [
        Server::streaming(&messages),
        Server::streaming(&completions),
        Server::streaming(&responses),
    ];
    let models = [
        cut[0].messages("m"),
        cut[1].completions("m", CompletionsDialect::Compatible),
        cut[2].responses("m", ResponsesDialect::Codex),
    ];
    for model in &models {
        let response = answer(model, &model.chat().user("hi").build())
            .await
            .expect("a cut answer should still finish");
        assert_eq!(response.stop(), &Stop::MaxTokens, "{model:?}");
        assert_eq!(response.text(), "half", "{model:?}");
        assert_eq!(response.tool_calls().count(), 0, "{model:?}");
    }

    let mut whole = messages;
    whole[7] = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}"#;
    let garbled = Server::streaming(&whole);
    let model = garbled.messages("m");
    let refused = answer(&model, &model.chat().user("hi").build()).await;
    assert!(matches!(refused, Err(Error::Malformed(_))), "{refused:?}");
}
