use rikka_llm::wire::{CloudCode, Transport, endpoint, headers};
use rikka_llm::{Effort, Message, ModelInfo, Stop, Tool, ToolResult, ToolUse, Vendor};
use serde_json::json;

use rikka_test_llm::Server;

use crate::support::{answer, sent};

#[tokio::test]
async fn request_is_wrapped_in_envelope() {
    let server = Server::streaming(&[]);
    let model = server.cloud_code("gemini-3-pro");
    let request = model
        .chat()
        .system("be brief")
        .cache_key("thread-1")
        .max_tokens(4096)
        .user("hi")
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["model"], json!("gemini-3-pro"));
    assert_eq!(body["project"], json!("projects/42"));
    assert_eq!(body["userAgent"], json!("antigravity"));
    assert_eq!(body["requestType"], json!("agent"));
    assert!(
        body["requestId"]
            .as_str()
            .is_some_and(|id| id.starts_with("agent-"))
    );
    assert_eq!(body["request"]["sessionId"], json!("thread-1"));
    assert_eq!(
        body["request"]["systemInstruction"],
        json!({ "role": "user", "parts": [{ "text": "be brief" }] })
    );
    assert_eq!(
        body["request"]["contents"],
        json!([{ "role": "user", "parts": [{ "text": "hi" }] }])
    );
    assert!(body["request"]["generationConfig"]["maxOutputTokens"].is_null());
}

#[tokio::test]
async fn tool_maps_to_function_declaration() {
    let server = Server::streaming(&[]);
    let model = server.cloud_code("claude-sonnet-4-6");
    let tool = Tool::new("get_weather")
        .description("Get current weather")
        .string("city", "City name")
        .string("units", "Units")
        .optional("units")
        .strict(true)
        .build()
        .expect("tool should be valid");
    let request = model.chat().tool(tool).max_tokens(4096).user("hi").build();
    let body = sent(&model, &request, &server).await;

    let declaration = &body["request"]["tools"][0]["functionDeclarations"][0];
    assert_eq!(declaration["name"], json!("get_weather"));
    assert_eq!(declaration["parameters"]["type"], json!("object"));
    assert_eq!(
        declaration["parameters"]["properties"]["units"]["nullable"],
        json!(true)
    );
    assert!(declaration["parameters"]["additionalProperties"].is_null());
    assert_eq!(
        body["request"]["toolConfig"]["functionCallingConfig"]["mode"],
        json!("VALIDATED")
    );
    assert_eq!(
        body["request"]["generationConfig"]["maxOutputTokens"],
        json!(4096)
    );
}

#[tokio::test]
async fn duplicate_call_ids_are_disambiguated() {
    let server = Server::streaming(&[]);
    let model = server.cloud_code("gemini-3-pro");
    let request = model
        .chat()
        .message(Message::tool_results([ToolResult::new("call_0", "lost")]))
        .user("read it then write it")
        .assistant(ToolUse::new("call_7", "read", json!({ "path": "a" })))
        .message(Message::tool_results([ToolResult::new(
            "call_7",
            "what a says",
        )]))
        .assistant(ToolUse::new("call_7", "write", json!({ "path": "b" })))
        .message(Message::tool_results([ToolResult::new(
            "call_7",
            "b was written",
        )]))
        .build();
    let contents = sent(&model, &request, &server).await["request"]["contents"].clone();

    assert_eq!(contents.as_array().map(Vec::len), Some(5));
    assert_eq!(
        contents[0]["parts"][0]["text"],
        json!("read it then write it")
    );
    let read = &contents[1]["parts"][0];
    let read_back = &contents[2]["parts"][0]["functionResponse"];
    let call = &contents[3]["parts"][0]["functionCall"];
    let written_back = &contents[4]["parts"][0]["functionResponse"];
    assert_eq!(
        read["thoughtSignature"],
        json!("skip_thought_signature_validator")
    );
    let read = &read["functionCall"];
    assert_ne!(read["id"], call["id"]);
    assert_eq!(read_back["id"], read["id"]);
    assert_eq!(read_back["name"], json!("read"));
    assert_eq!(written_back["id"], call["id"]);
    assert_eq!(written_back["name"], json!("write"));
}

#[tokio::test]
async fn partial_candidates_merge() {
    let server = Server::streaming(&[
        r#"{"response":{"responseId":"r1","modelVersion":"gemini-3-pro","candidates":[{"content":{"parts":[{"text":"Hel"}]}}]}}"#,
        r#"{"response":{"responseId":"r1","modelVersion":"gemini-3-pro","candidates":[{"content":{"parts":[{"text":"lo"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":2,"thoughtsTokenCount":3,"cachedContentTokenCount":4}}}"#,
    ]);
    let model = server.cloud_code("gemini-3-pro");
    let response = answer(&model, &model.chat().user("hi").build())
        .await
        .expect("stream should finish");

    assert_eq!(response.text(), "Hello");
    assert_eq!(response.stop(), &Stop::End);
    assert_eq!(response.usage().cache_read, 4);
    assert_eq!(response.usage().input, 6);
    assert_eq!(response.usage().output, 5);
    assert_eq!(response.usage().reasoning, 3);
}

#[tokio::test]
async fn call_signature_is_replayed() {
    let signing = Server::streaming(&[
        r#"{"response":{"responseId":"r4","modelVersion":"gemini-3-pro","candidates":[{"content":{"parts":[{"text":"looking","thought":true},{"functionCall":{"name":"read","args":{"path":"a"}},"thoughtSignature":"sig"},{"functionCall":{"name":"read","args":{"path":"b"}}}]},"finishReason":"STOP"}]}}"#,
    ]);
    let model = signing.cloud_code("gemini-3-pro");
    let response = answer(&model, &model.chat().user("read them").build())
        .await
        .expect("stream should finish");

    assert_eq!(response.stop(), &Stop::ToolUse);
    assert_eq!(response.reasoning().count(), 1);
    let calls: Vec<_> = response.tool_calls().collect();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].is_signed(Vendor::Antigravity));
    assert!(!calls[1].is_signed(Vendor::Antigravity));
    assert_eq!(calls[0].id(), "read-0");
    assert_eq!(calls[1].id(), "read-1");
    assert_eq!(calls[0].input(), &json!({ "path": "a" }));

    let replaying = Server::streaming(&[]);
    let model = replaying.cloud_code("gemini-3-pro");
    let request = model
        .chat()
        .user("read them")
        .message(response.into_message())
        .build();
    let body = sent(&model, &request, &replaying).await;
    let parts = &body["request"]["contents"][1]["parts"];
    let signed = parts
        .as_array()
        .expect("turn should have parts")
        .iter()
        .filter(|part| part.get("functionCall").is_some())
        .map(|part| part["thoughtSignature"].clone())
        .collect::<Vec<_>>();
    assert_eq!(signed, [json!("sig"), json!(null)]);
}

#[tokio::test]
async fn truncated_stream_is_error() {
    let server = Server::streaming(&[
        r#"{"response":{"responseId":"r3","modelVersion":"gemini-3-pro","candidates":[{"content":{"parts":[{"text":"half"}]}}]}}"#,
    ]);
    let model = server.cloud_code("gemini-3-pro");

    assert!(
        answer(&model, &model.chat().user("hi").build())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reasoning_level_picks_the_listed_variant() {
    let listed =
        ModelInfo::new("gemini-4-hydrogen", "Gemini 4 Hydrogen", 1_048_576, 262_144).with_efforts(
            vec![Effort::Low, Effort::Medium, Effort::High, Effort::XHigh],
        );
    for (effort, variant) in [
        (Some(Effort::XHigh), "gemini-4-hydrogen-xhigh"),
        (Some(Effort::Max), "gemini-4-hydrogen-xhigh"),
        (Some(Effort::Low), "gemini-4-hydrogen-low"),
        (Some(Effort::Off), "gemini-4-hydrogen-low"),
        (None, "gemini-4-hydrogen-medium"),
    ] {
        let server = Server::streaming(&[]);
        let transport = Transport::new(
            endpoint(server.url()).expect("test server URL should parse"),
            headers().build().expect("no headers should build"),
        )
        .expect("test transport should build");
        let model = CloudCode::new(transport, listed.id())
            .listed(&listed)
            .project("projects/42")
            .build();
        let request = match effort {
            Some(effort) => model.chat().effort(effort),
            None => model.chat(),
        }
        .user("hi")
        .build();

        let body = sent(&model, &request, &server).await;

        assert_eq!(body["model"], json!(variant), "{effort:?}");
    }
}
