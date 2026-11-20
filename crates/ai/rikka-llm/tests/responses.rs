use rikka_llm::wire::ResponsesDialect;
use rikka_llm::{
    Cache, Content, Effort, Error, Event, Message, Reveal, Schema, Stop, Tool, ToolResult, ToolUse,
    Vendor,
};
use serde_json::json;

use rikka_test_llm::Server;

use crate::support::{answer, events, finished, sent};

#[tokio::test]
async fn requests_disable_server_storage() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.2", ResponsesDialect::OpenAi);
    let request = model
        .chat()
        .system("be brief")
        .effort(Effort::XHigh)
        .reveal(Reveal::Summary)
        .user("hello")
        .assistant(vec![
            Content::text("checking"),
            Content::ToolUse(ToolUse::new(
                "call_1",
                "get_weather",
                json!({ "location": "Tokyo" }),
            )),
        ])
        .message(Message::tool_results([ToolResult::new("call_1", "28C")]))
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["store"], json!(false));
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(body["instructions"], json!("be brief"));
    assert_eq!(
        body["reasoning"],
        json!({ "effort": "xhigh", "summary": "auto" })
    );
    let input = &body["input"];
    assert_eq!(
        input[0],
        json!({
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": "hello" }],
        })
    );
    assert_eq!(input[1]["role"], json!("assistant"));
    assert_eq!(input[1]["content"][0]["type"], json!("output_text"));
    assert_eq!(input[2]["type"], json!("function_call"));
    assert_eq!(input[2]["call_id"], json!("call_1"));
    assert_eq!(input[2]["arguments"], json!(r#"{"location":"Tokyo"}"#));
    assert_eq!(input[3]["type"], json!("function_call_output"));
    assert_eq!(input[3]["call_id"], json!("call_1"));
}

#[tokio::test]
async fn strict_tool_requires_all_properties() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.2", ResponsesDialect::OpenAi);
    let tool = Tool::new("search")
        .description("Search")
        .string("query", "What to look for")
        .param("filter", Schema::object().optional("key", Schema::string()))
        .optional("filter")
        .strict(true)
        .build()
        .expect("tool should be valid");
    let body = sent(&model, &model.chat().tool(tool).user("hi").build(), &server).await;

    let declared = &body["tools"][0];
    assert_eq!(declared["type"], json!("function"));
    assert_eq!(declared["strict"], json!(true));
    let parameters = &declared["parameters"];
    assert_eq!(parameters["additionalProperties"], json!(false));
    assert_eq!(parameters["required"], json!(["query", "filter"]));
    let filter = &parameters["properties"]["filter"];
    assert_eq!(filter["type"], json!(["object", "null"]));
    assert_eq!(filter["additionalProperties"], json!(false));
    assert_eq!(filter["required"], json!(["key"]));
    assert_eq!(
        filter["properties"]["key"]["type"],
        json!(["string", "null"])
    );

    assert!(
        Tool::new("get_weather")
            .string("location", "City name")
            .optional("unti")
            .build()
            .is_err(),
        "a parameter that was never declared cannot be optional"
    );
}

#[tokio::test]
async fn last_tool_results_are_cache_marked() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.2", ResponsesDialect::OpenAi);
    let request = model
        .chat()
        .cache(Cache::Conversation)
        .user("weather?")
        .assistant(Content::ToolUse(ToolUse::new(
            "call_1",
            "get_weather",
            json!({ "location": "Tokyo" }),
        )))
        .message(Message::tool_results([ToolResult::new("call_1", "28C")]))
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["prompt_cache_options"], json!({ "mode": "explicit" }));
    assert_eq!(
        body["input"][2]["output"][0]["prompt_cache_breakpoint"],
        json!({ "mode": "explicit" })
    );
}

#[tokio::test]
async fn stop_sequences_are_rejected() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.2", ResponsesDialect::OpenAi);
    let refused = answer(&model, &model.chat().stop("END").user("hi").build()).await;

    assert!(matches!(refused, Err(Error::Unsupported { .. })));
}

#[tokio::test]
async fn unsupported_fields_are_omitted() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.2-codex", ResponsesDialect::Codex);
    let request = model
        .chat()
        .cache(Cache::Conversation)
        .system("be brief")
        .user("hi")
        .max_tokens(1024)
        .temperature(0.5)
        .top_p(0.9)
        .build();
    let body = sent(&model, &request, &server).await;

    for refused in [
        "max_output_tokens",
        "temperature",
        "top_p",
        "prompt_cache_options",
    ] {
        assert!(body.get(refused).is_none(), "{refused} was sent");
    }
    assert!(!body.to_string().contains("prompt_cache_breakpoint"));
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["store"], json!(false));
    assert_eq!(body["instructions"], json!("be brief"));
}

#[tokio::test]
async fn response_is_built_from_completed_object() {
    let server = Server::streaming(&[
        r#"{"type":"response.created","response":{"id":"resp_1","model":"gpt-5.2"}}"#,
        r#"{"type":"response.output_text.delta","delta":"Hel"}"#,
        r#"{"type":"response.output_text.delta","delta":"lo"}"#,
        r#"{"type":"response.completed","response":{"id":"resp_1","model":"gpt-5.2","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello"}]}],"usage":{"input_tokens":30,"input_tokens_details":{"cached_tokens":10,"cache_write_tokens":0},"output_tokens":5,"output_tokens_details":{"reasoning_tokens":2}}}}"#,
    ]);
    let model = server.responses("gpt-5.2", ResponsesDialect::Codex);
    let events = events(&model, &model.chat().user("hi").build()).await;

    assert_eq!(
        events.first(),
        Some(&Event::Start {
            id: "resp_1".to_owned(),
            model: "gpt-5.2".to_owned(),
        })
    );
    let response = finished(&events);
    assert_eq!(response.text(), "Hello");
    assert_eq!(response.stop(), &Stop::End);
    assert_eq!(response.usage().input, 20);
    assert_eq!(response.usage().cache_read, 10);
    assert_eq!(response.usage().reasoning, 2);
}

#[tokio::test]
async fn usage_only_close_uses_streamed_items() {
    let server = Server::streaming(&[
        r#"{"type":"response.created","response":{"id":"resp_5","model":"gpt-5.2-codex"}}"#,
        r#"{"type":"response.output_item.done","item":{"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"gAAA"},"output_index":0}"#,
        r#"{"type":"response.output_item.added","item":{"type":"function_call","call_id":"call_1","name":"read"},"output_index":1}"#,
        r#"{"type":"response.function_call_arguments.delta","delta":"{\"path\":","output_index":1}"#,
        r#"{"type":"response.function_call_arguments.delta","delta":" \"a\"}","output_index":1}"#,
        r#"{"type":"response.output_item.done","item":{"type":"function_call","call_id":"call_1","name":"read","arguments":"{\"path\": \"a\"}"},"output_index":1}"#,
        r#"{"type":"response.completed","response":{"id":"resp_5","usage":{"input_tokens":9,"output_tokens":4}}}"#,
    ]);
    let model = server.responses("gpt-5.2-codex", ResponsesDialect::Codex);
    let events = events(&model, &model.chat().user("hi").build()).await;

    assert!(events.contains(&Event::ToolStart {
        index: 0,
        id: "call_1".to_owned(),
        name: "read".to_owned(),
    }));
    let response = finished(&events);
    assert_eq!(response.model(), "gpt-5.2-codex");
    assert_eq!(response.stop(), &Stop::ToolUse);
    assert_eq!(
        response
            .tool_calls()
            .next()
            .map(|call| call.input().clone()),
        Some(json!({ "path": "a" }))
    );
    assert!(
        response
            .reasoning()
            .next()
            .expect("reasoning should be kept")
            .is_replayable(Vendor::OpenAi)
    );
    assert_eq!(response.usage().input, 9);
}

#[tokio::test]
async fn truncation_and_refusal_are_reported() {
    for (frame, stop) in [
        (
            r#"{"type":"response.incomplete","response":{"id":"resp_3","model":"gpt-5.2","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"half"}]}]}}"#,
            Stop::MaxTokens,
        ),
        (
            r#"{"type":"response.completed","response":{"id":"resp_4","model":"gpt-5.2","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"refusal","refusal":"I cannot help with that"}]}]}}"#,
            Stop::Declined("I cannot help with that".to_owned()),
        ),
    ] {
        let server = Server::streaming(&[frame]);
        let model = server.responses("gpt-5.2", ResponsesDialect::Codex);
        let response = answer(&model, &model.chat().user("hi").build())
            .await
            .expect("stream should finish");
        assert_eq!(response.stop(), &stop);
    }
}
