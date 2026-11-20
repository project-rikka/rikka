use rikka_llm::wire::CompletionsDialect;
use rikka_llm::{Content, Effort, Event, Image, Message, Stop, ToolResult, ToolUse, Vendor};
use serde_json::json;

use rikka_test_llm::Server;

use crate::support::{answer, events, finished, sent};

#[tokio::test]
async fn conversation_maps_to_messages() {
    let server = Server::streaming(&[]);
    let model = server.completions("gemini-3-pro", CompletionsDialect::Compatible);
    let request = model
        .chat()
        .system("be brief")
        .effort(Effort::XHigh)
        .max_tokens(1024)
        .user(vec![
            Content::text("what is this"),
            Content::Image(Image::png([0x89, 0x50])),
        ])
        .assistant(vec![
            Content::text("checking"),
            Content::ToolUse(ToolUse::new(
                "call_1",
                "get_weather",
                json!({ "location": "Tokyo" }),
            )),
        ])
        .message(Message::tool_results([
            ToolResult::new("call_1", "28C").image(Image::png([0x89])),
            ToolResult::new("call_2", "and here"),
        ]))
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["stream_options"], json!({ "include_usage": true }));
    assert_eq!(body["reasoning_effort"], json!("xhigh"));
    assert_eq!(body["max_completion_tokens"], json!(1024));
    let messages = &body["messages"];
    assert_eq!(
        messages[0],
        json!({ "role": "system", "content": "be brief" })
    );
    assert_eq!(
        messages[1]["content"][0],
        json!({ "type": "text", "text": "what is this" })
    );
    assert_eq!(
        messages[1]["content"][1]["image_url"]["url"],
        json!("data:image/png;base64,iVA=")
    );
    assert_eq!(messages[2]["content"], json!("checking"));
    assert_eq!(messages[2]["tool_calls"][0]["id"], json!("call_1"));
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["arguments"],
        json!(r#"{"location":"Tokyo"}"#)
    );
    assert_eq!(
        messages[3],
        json!({ "role": "tool", "tool_call_id": "call_1", "content": "28C" })
    );
    assert_eq!(messages[4]["role"], json!("tool"));
    assert_eq!(messages[5]["role"], json!("user"));
    assert_eq!(messages[5]["content"][0]["type"], json!("image_url"));
}

#[tokio::test]
async fn text_chunks_assemble_answer() {
    let server = Server::streaming(&[
        r#"{"id":"chatcmpl-1","model":"gemini-3-pro","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"}}]}"#,
        r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"lo"}}]}"#,
        r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
        r#"{"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":30,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":10}}}"#,
        "[DONE]",
        "[DONE]",
    ]);
    let model = server.completions("gemini-3-pro", CompletionsDialect::Compatible);
    let events = events(&model, &model.chat().user("hi").build()).await;

    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::Done(_)))
            .count(),
        1
    );
    let response = finished(&events);
    assert_eq!(response.text(), "Hello");
    assert_eq!(response.stop(), &Stop::End);
    assert_eq!(response.usage().input, 20);
    assert_eq!(response.usage().cache_read, 10);
}

#[tokio::test]
async fn tool_call_is_announced_once() {
    let server = Server::streaming(&[
        r#"{"id":"chatcmpl-2","model":"m","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"get_weather","arguments":""}}]}}]}"#,
        r#"{"id":"chatcmpl-2","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\":"}}]}}]}"#,
        r#"{"id":"chatcmpl-2","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":" \"Tokyo\"}"}}]}}]}"#,
        r#"{"id":"chatcmpl-2","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        "[DONE]",
    ]);
    let model = server.completions("m", CompletionsDialect::Compatible);
    let events = events(&model, &model.chat().user("hi").build()).await;

    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::ToolStart { .. }))
            .count(),
        1
    );
    let response = finished(&events);
    assert_eq!(response.stop(), &Stop::ToolUse);
    let call = response
        .tool_calls()
        .next()
        .expect("call should be complete");
    assert_eq!(call.id(), "call_1");
    assert_eq!(call.input(), &json!({ "location": "Tokyo" }));
}

#[tokio::test]
async fn reasoning_details_round_trip() {
    let thinking = Server::streaming(&[
        r#"{"id":"gen-1","model":"anthropic/claude-opus-5","choices":[{"index":0,"delta":{"reasoning":"wor","reasoning_details":[{"type":"reasoning.text","text":"wor","format":"anthropic-claude-v1","index":0}]}}]}"#,
        r#"{"id":"gen-1","choices":[{"index":0,"delta":{"reasoning":"king","reasoning_details":[{"type":"reasoning.text","text":"king","signature":"notarealsignature","index":0}]}}]}"#,
        r#"{"id":"gen-1","choices":[{"index":0,"delta":{"content":"done"},"finish_reason":"stop"}]}"#,
        "[DONE]",
    ]);
    let model = thinking.completions("anthropic/claude-opus-5", CompletionsDialect::OpenRouter);
    let response = answer(&model, &model.chat().user("hi").build())
        .await
        .expect("stream should finish");
    let block = response
        .reasoning()
        .next()
        .expect("block should be complete");
    assert_eq!(block.text(), "working");
    assert!(block.is_replayable(Vendor::OpenRouter));
    assert!(!block.is_replayable(Vendor::Compatible));
    let message = response.into_message();

    let routed = Server::streaming(&[]);
    let model = routed.completions("anthropic/claude-opus-5", CompletionsDialect::OpenRouter);
    let body = sent(
        &model,
        &model
            .chat()
            .user("hi")
            .message(message.clone())
            .user("and?")
            .build(),
        &routed,
    )
    .await;
    assert_eq!(body["reasoning"], json!(null));
    assert_eq!(
        body["messages"][1]["reasoning_details"],
        json!([{
            "type": "reasoning.text",
            "text": "working",
            "signature": "notarealsignature",
            "format": "anthropic-claude-v1",
            "index": 0,
        }])
    );

    let plain = Server::streaming(&[]);
    let model = plain.completions("m", CompletionsDialect::Compatible);
    let body = sent(
        &model,
        &model
            .chat()
            .user("hi")
            .message(message)
            .user("and?")
            .build(),
        &plain,
    )
    .await;
    assert!(body["messages"][1].get("reasoning_details").is_none());
    assert!(body["messages"][1].get("reasoning_content").is_none());
    assert_eq!(body["messages"][1]["content"], json!("done"));
}

#[tokio::test]
async fn stream_without_done_returns_content() {
    let server = Server::streaming(&[
        r#"{"id":"chatcmpl-4","model":"m","choices":[{"index":0,"delta":{"content":"half"},"finish_reason":"stop"}]}"#,
    ]);
    let model = server.completions("m", CompletionsDialect::Compatible);
    let response = answer(&model, &model.chat().user("hi").build())
        .await
        .expect("answer should be returned");

    assert_eq!(response.text(), "half");
}

#[tokio::test]
async fn error_chunk_becomes_error() {
    let server =
        Server::streaming(&[r#"{"error":{"message":"model is loading","type":"server_error"}}"#]);
    let model = server.completions("m", CompletionsDialect::Compatible);
    let error = answer(&model, &model.chat().user("hi").build())
        .await
        .expect_err("the stream carried a failure");

    assert_eq!(
        error.to_string(),
        "provider rejected the request: model is loading"
    );
}
