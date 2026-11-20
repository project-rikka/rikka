use rikka_llm::wire::{CompletionsDialect, ResponsesDialect};
use rikka_llm::{Message, Model, Request, Tool, ToolResult, ToolUse};
use serde_json::{Value, json};

use rikka_test_llm::Server;

use crate::support::sent;

fn tool(name: &str) -> Tool {
    Tool::new(name)
        .description(format!("Does {name}"))
        .string("what", "What to act on")
        .build()
        .expect("tool should be valid")
}

fn loading(model: &Model) -> Request {
    model
        .chat()
        .tool(tool("search"))
        .tool(tool("weather").deferred())
        .tool(tool("calendar").deferred())
        .user("weather in Tokyo?")
        .assistant(ToolUse::new(
            "call_1",
            "search",
            json!({ "what": "weather" }),
        ))
        .message(Message::tool_results([ToolResult::new("call_1", "found")
            .reference("weather")
            .reference("gone")]))
        .build()
}

fn named(tools: &Value) -> Vec<&str> {
    tools
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["name"].as_str().or(tool["function"]["name"].as_str()))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn messages_declare_deferred_tools() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let body = sent(&model, &loading(&model), &server).await;

    assert_eq!(named(&body["tools"]), ["search", "weather", "calendar"]);
    assert!(body["tools"][0].get("defer_loading").is_none());
    assert_eq!(body["tools"][1]["defer_loading"], json!(true));
    assert_eq!(body["tools"][2]["defer_loading"], json!(true));
}

#[tokio::test]
async fn messages_keep_references_alone_in_their_result() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let request = model
        .chat()
        .tool(tool("search"))
        .tool(tool("weather").deferred())
        .user("weather in Tokyo?")
        .message(
            Message::assistant(ToolUse::new(
                "call_1",
                "search",
                json!({ "what": "weather" }),
            ))
            .with(ToolUse::new("call_2", "search", json!({ "what": "Tokyo" }))),
        )
        .message(Message::tool_results([
            ToolResult::new("call_1", "found").reference("weather"),
            ToolResult::new("call_2", "Tokyo is in Japan"),
        ]))
        .build();
    let body = sent(&model, &request, &server).await;
    let content = &body["messages"][2]["content"];

    assert_eq!(content[0]["tool_use_id"], json!("call_1"));
    assert_eq!(
        content[0]["content"],
        json!([{ "type": "tool_reference", "tool_name": "weather" }])
    );
    assert_eq!(content[1]["tool_use_id"], json!("call_2"));
    assert_eq!(
        content[1]["content"],
        json!([{ "type": "text", "text": "Tokyo is in Japan" }])
    );
    assert_eq!(content[2]["type"], json!("text"));
    assert_eq!(content[2]["text"], json!("found"));
    assert_eq!(content.as_array().map(Vec::len), Some(3));
}

#[tokio::test]
async fn responses_insert_loaded_tool_in_place() {
    let server = Server::streaming(&[]);
    let model = server.responses("gpt-5.5", ResponsesDialect::OpenAi);
    let body = sent(&model, &loading(&model), &server).await;

    assert_eq!(named(&body["tools"]), ["search"]);
    let input = body["input"].as_array().expect("input should be listed");
    let output = input
        .iter()
        .position(|item| item["type"] == json!("function_call_output"))
        .expect("call should succeed");
    assert_eq!(
        input[output]["output"],
        json!([{ "type": "input_text", "text": "found" }])
    );
    assert_eq!(input[output + 1]["type"], json!("additional_tools"));
    assert_eq!(input[output + 1]["role"], json!("developer"));
    assert_eq!(named(&input[output + 1]["tools"]), ["weather"]);
}

#[tokio::test]
async fn protocols_without_deferral_declare_loaded_only() {
    let server = Server::streaming(&[]);
    let models = [
        server.responses("gpt-5.5", ResponsesDialect::Codex),
        server.completions("deepseek-chat", CompletionsDialect::Compatible),
        server.cloud_code("gemini-3-pro"),
    ];

    for model in models {
        let body = sent(&model, &loading(&model), &server).await;
        let tools = match body["request"]["tools"][0]["functionDeclarations"].is_array() {
            true => &body["request"]["tools"][0]["functionDeclarations"],
            false => &body["tools"],
        };
        assert_eq!(named(tools), ["search", "weather"], "{}", model.name());
        assert!(!body.to_string().contains("additional_tools"));
        assert!(!body.to_string().contains("defer_loading"));
    }
}
