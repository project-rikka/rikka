use rikka_llm::wire::{Messages, Transport, endpoint, headers};
use rikka_llm::{
    Cache, Content, Effort, Error, Event, Image, Message, ModelInfo, Reveal, Stop, ThinkingOff,
    Tool, ToolResult, ToolUse, Ttl, Vendor,
};
use serde_json::json;

use rikka_test_llm::Server;

use crate::support::{answer, events, finished, sent};

fn weather() -> Tool {
    Tool::new("get_weather")
        .description("Get current weather")
        .string("location", "City name")
        .build()
        .expect("tool should be valid")
}

#[tokio::test]
async fn tool_use_turn_round_trips() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let request = model
        .chat()
        .tool(weather())
        .user(vec![
            Content::text("weather in Tokyo?"),
            Content::Image(Image::png([0x89, 0x50])),
        ])
        .assistant(Content::ToolUse(ToolUse::new(
            "toolu_1",
            "get_weather",
            json!({ "location": "Tokyo" }),
        )))
        .message(Message::tool_results([
            ToolResult::new("toolu_1", "28C"),
            ToolResult::failed("toolu_2", "no such city"),
        ]))
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["model"], json!("claude-opus-5"));
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["tools"][0]["name"], json!("get_weather"));
    assert_eq!(
        body["tools"][0]["input_schema"]["required"],
        json!(["location"])
    );
    assert_eq!(body["tool_choice"], json!({ "type": "auto" }));
    let picture = &body["messages"][0]["content"][1]["source"];
    assert_eq!(picture["type"], json!("base64"));
    assert_eq!(picture["media_type"], json!("image/png"));
    assert_eq!(picture["data"], json!("iVA="));
    assert_eq!(body["messages"][1]["content"][0]["id"], json!("toolu_1"));
    assert_eq!(
        body["messages"][2]["content"][0],
        json!({
            "type": "tool_result",
            "tool_use_id": "toolu_1",
            "content": [{ "type": "text", "text": "28C" }],
        })
    );
    assert_eq!(body["messages"][2]["content"][1]["is_error"], json!(true));
}

#[tokio::test]
async fn effort_maps_to_thinking_config() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");

    let thinking = model
        .chat()
        .effort(Effort::XHigh)
        .reveal(Reveal::Summary)
        .user("hi")
        .build();
    let body = sent(&model, &thinking, &server).await;
    assert_eq!(
        body["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(body["output_config"]["effort"], json!("xhigh"));

    let off = model.chat().effort(Effort::Off).user("hi").build();
    let body = sent(&model, &off, &server).await;
    assert_eq!(body["thinking"], json!({ "type": "disabled" }));
    assert!(body.get("output_config").is_none());
}

#[tokio::test]
async fn thinking_off_follows_model() {
    let server = Server::streaming(&[]);
    let transport = Transport::new(
        endpoint(server.url()).expect("test server URL should parse"),
        headers().build().expect("headers should build"),
    )
    .expect("transport should build");
    let listed = ModelInfo::new("claude-sonnet-5-5", "Claude Sonnet 5.5", 1_000_000, 128_000)
        .with_thinking_off(ThinkingOff::BetweenTools);
    let model = Messages::new(transport, listed.id())
        .listed(&listed)
        .build();

    let off = model
        .chat()
        .effort(Effort::Off)
        .reveal(Reveal::Summary)
        .user("hi")
        .build();
    let body = sent(&model, &off, &server).await;
    assert_eq!(body["thinking"], json!({ "type": "between_tools" }));
    assert!(body.get("output_config").is_none());
}

#[tokio::test]
async fn sampling_parameters_are_refused() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let request = model.chat().temperature(0.2).user("hi").build();

    let refused = answer(&model, &request).await;

    assert!(matches!(refused, Err(Error::Unsupported { .. })));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn cache_policy_marks_prefix_and_last_user_turns() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let request = model
        .chat()
        .cache(Cache::Conversation)
        .ttl(Ttl::Long)
        .system("instructions")
        .tool(weather())
        .user("one")
        .assistant("two")
        .user("three")
        .build();
    let body = sent(&model, &request, &server).await;
    let hour = json!({ "type": "ephemeral", "ttl": "1h" });

    assert_eq!(body["system"][0]["cache_control"], hour);
    assert!(body["tools"][0].get("cache_control").is_none());
    assert_eq!(body["messages"][0]["content"][0]["cache_control"], hour);
    assert!(
        body["messages"][1]["content"][0]
            .get("cache_control")
            .is_none()
    );
    assert_eq!(body["messages"][2]["content"][0]["cache_control"], hour);
}

#[tokio::test]
async fn text_deltas_assemble_answer() {
    let server = Server::streaming(&[
        r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-opus-5","usage":{"input_tokens":25,"cache_read_input_tokens":10}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"ping"}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"lo"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    let model = server.messages("claude-opus-5");
    let events = events(&model, &model.chat().user("hi").build()).await;

    assert_eq!(
        events.first(),
        Some(&Event::Start {
            id: "msg_1".to_owned(),
            model: "claude-opus-5".to_owned(),
        })
    );
    assert_eq!(
        events.iter().filter_map(Event::as_text).collect::<String>(),
        "Hello"
    );
    let response = finished(&events);
    assert_eq!(response.text(), "Hello");
    assert_eq!(response.stop(), &Stop::End);
    assert_eq!(response.usage().input, 25);
    assert_eq!(response.usage().cache_read, 10);
    assert_eq!(response.usage().output, 15);
}

#[tokio::test]
async fn tool_call_completes_when_arguments_parse() {
    let server = Server::streaming(&[
        r#"{"type":"message_start","message":{"id":"msg_2","model":"claude-opus-5","usage":{}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"checking"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"get_weather","input":{}}}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"get_weather","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" \"Tokyo\"}"}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
        r#"{"type":"content_block_start","index":4000000000,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":4000000000,"delta":{"type":"text_delta","text":"nowhere"}}"#,
        r#"{"type":"content_block_stop","index":4000000000}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":89}}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    let model = server.messages("claude-opus-5");
    let events = events(&model, &model.chat().user("hi").build()).await;

    let announced: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, Event::ToolStart { .. }))
        .collect();
    assert_eq!(
        announced,
        [&Event::ToolStart {
            index: 0,
            id: "toolu_1".to_owned(),
            name: "get_weather".to_owned(),
        }]
    );

    let response = finished(&events);
    assert_eq!(response.stop(), &Stop::ToolUse);
    assert_eq!(response.text(), "checking");
    assert_eq!(response.content().len(), 2);
    let calls: Vec<_> = response.tool_calls().collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].input(), &json!({ "location": "Tokyo" }));
}

#[tokio::test]
async fn signed_thinking_is_replayed_verbatim() {
    let thinking = Server::streaming(&[
        r#"{"type":"message_start","message":{"id":"msg_3","model":"claude-opus-5","usage":{}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"working"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"EqQB"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    let model = thinking.messages("claude-opus-5");
    let response = answer(&model, &model.chat().user("hi").build())
        .await
        .expect("stream should finish");
    let reasoning = response.reasoning().next().expect("reasoning should exist");
    assert_eq!(reasoning.text(), "working");
    assert!(reasoning.is_replayable(Vendor::Anthropic));
    assert!(!reasoning.is_replayable(Vendor::OpenAi));

    let next = Server::streaming(&[]);
    let model = next.messages("claude-opus-5");
    let request = model
        .chat()
        .cache(Cache::Conversation)
        .user("hi")
        .message(response.into_message())
        .user("go on")
        .build();
    let body = sent(&model, &request, &next).await;

    let replayed = &body["messages"][1]["content"][0];
    assert_eq!(replayed["type"], json!("thinking"));
    assert_eq!(replayed["thinking"], json!("working"));
    assert_eq!(replayed["signature"], json!("EqQB"));
    assert!(replayed.get("cache_control").is_none());
}

#[tokio::test]
async fn blank_text_blocks_are_omitted() {
    let server = Server::streaming(&[]);
    let model = server.messages("claude-opus-5");
    let request = model
        .chat()
        .user("weather?")
        .assistant(vec![
            Content::text(""),
            Content::text("Rain"),
            Content::text(" "),
            Content::text(""),
            Content::text("tonight."),
        ])
        .user("and tomorrow?")
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(
        body["messages"][1]["content"],
        json!([
            { "type": "text", "text": "Rain " },
            { "type": "text", "text": "tonight." },
        ])
    );
}

#[tokio::test]
async fn failed_or_truncated_stream_is_error() {
    let failing = Server::streaming(&[
        r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
    ]);
    let model = failing.messages("claude-opus-5");
    assert!(matches!(
        answer(&model, &model.chat().user("hi").build()).await,
        Err(Error::Overloaded)
    ));

    let short = Server::streaming(&[
        r#"{"type":"message_start","message":{"id":"msg_4","model":"m","usage":{}}}"#,
    ]);
    let model = short.messages("claude-opus-5");
    assert!(
        answer(&model, &model.chat().user("hi").build())
            .await
            .is_err()
    );
}
