use rikka_llm::wire::{CompletionsDialect, Messages, Transport, endpoint, headers};
use rikka_llm::{
    Cache, Content, Event, Hosted, HostedOutcome, HostedPart, Model, ModelInfo, ServerTool, Stop,
    Tool,
};
use rikka_test_llm::{Exchange, Reply, Server};
use serde_json::{Value, json};

use crate::support::{events, finished, sent};

fn hosting(server: &Server, tools: &[ServerTool]) -> Model {
    let transport = Transport::new(
        endpoint(server.url()).expect("test server URL should parse"),
        headers().build().expect("headers should build"),
    )
    .expect("transport should build");
    let listed = ModelInfo::new("claude-opus-5-5", "Claude Opus 5.5", 1_000_000, 128_000)
        .with_server_tools(tools.to_vec());
    Messages::new(transport, listed.id())
        .listed(&listed)
        .build()
}

fn lookup() -> Tool {
    Tool::new("lookup")
        .description("Look something up")
        .string("query", "What to find")
        .build()
        .expect("tool should be valid")
}

#[tokio::test]
async fn server_tools_are_declared_only_where_model_hosts_them() {
    let cases: [(&[ServerTool], Option<&str>, Value); 4] = [
        (
            &ServerTool::ALL,
            None,
            json!([
                { "type": "web_search_20250305", "name": "web_search" },
                {
                    "type": "web_fetch_20250910",
                    "name": "web_fetch",
                    "max_content_tokens": 50_000,
                    "cache_control": { "type": "ephemeral" },
                },
            ]),
        ),
        (
            &[ServerTool::WebFetch],
            None,
            json!([{
                "type": "web_fetch_20250910",
                "name": "web_fetch",
                "max_content_tokens": 50_000,
                "cache_control": { "type": "ephemeral" },
            }]),
        ),
        (&[], None, Value::Null),
        (&ServerTool::ALL, Some("claude-haiku-5-5"), Value::Null),
    ];

    for (hosted, model_override, declared) in cases {
        let server = Server::streaming(&[]);
        let model = hosting(&server, hosted);
        let mut chat = model
            .chat()
            .cache(Cache::Conversation)
            .server_tools(ServerTool::ALL)
            .user("what changed in rust this year?");
        if let Some(name) = model_override {
            chat = chat.model(name);
        }
        let body = sent(&model, &chat.build(), &server).await;

        assert_eq!(body["tools"], declared, "{hosted:?} {model_override:?}");
    }
}

#[tokio::test]
async fn server_tools_follow_client_tools_and_take_cache_mark() {
    let server = Server::streaming(&[]);
    let model = hosting(&server, &[ServerTool::WebSearch]);
    let request = model
        .chat()
        .cache(Cache::Conversation)
        .tool(lookup())
        .server_tools([ServerTool::WebSearch])
        .user("hi")
        .build();
    let body = sent(&model, &request, &server).await;

    assert_eq!(body["tools"][0]["name"], json!("lookup"));
    assert!(body["tools"][0].get("cache_control").is_none());
    assert_eq!(
        body["tools"][1],
        json!({
            "type": "web_search_20250305",
            "name": "web_search",
            "cache_control": { "type": "ephemeral" },
        })
    );
    assert_eq!(body["tool_choice"], json!({ "type": "auto" }));
}

#[tokio::test]
async fn web_search_blocks_stream_and_replay_verbatim() {
    let result = json!({
        "type": "web_search_tool_result",
        "tool_use_id": "srvtoolu_1",
        "content": [{
            "type": "web_search_result",
            "url": "https://blog.rust-lang.org/",
            "title": "Rust Blog",
            "encrypted_content": "EqgfCioIARgB",
            "page_age": "May 1, 2026",
        }],
    });
    let frames = [
        json!({ "type": "message_start", "message": { "id": "msg_1", "model": "claude-opus-5-5", "usage": {} } }),
        json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
        json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Let me look." } }),
        json!({ "type": "content_block_stop", "index": 0 }),
        json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "server_tool_use", "id": "srvtoolu_1", "name": "web_search", "input": {} } }),
        json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "input_json_delta", "partial_json": "{\"query\":" } }),
        json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "input_json_delta", "partial_json": "\"rust 2026\"}" } }),
        json!({ "type": "content_block_stop", "index": 1 }),
        json!({ "type": "content_block_start", "index": 2, "content_block": result }),
        json!({ "type": "content_block_stop", "index": 2 }),
        json!({ "type": "content_block_start", "index": 3, "content_block": { "type": "text", "text": "" } }),
        json!({ "type": "content_block_delta", "index": 3, "delta": { "type": "text_delta", "text": "Found it." } }),
        json!({ "type": "content_block_stop", "index": 3 }),
        json!({ "type": "message_delta", "delta": { "stop_reason": "pause_turn" }, "usage": { "output_tokens": 9 } }),
        json!({ "type": "message_stop" }),
    ]
    .map(|frame| frame.to_string());
    let server = Server::streaming(&frames.iter().map(String::as_str).collect::<Vec<_>>());
    let model = hosting(&server, &ServerTool::ALL);
    let request = model
        .chat()
        .server_tools(ServerTool::ALL)
        .user("what changed in rust this year?")
        .build();

    let events = events(&model, &request).await;
    let parts: Vec<&HostedPart> = events
        .iter()
        .filter_map(|event| match event {
            Event::Hosted(part) => Some(part),
            _ => None,
        })
        .collect();
    assert_eq!(
        parts[0],
        &HostedPart::Call {
            id: "srvtoolu_1".to_owned(),
            tool: ServerTool::WebSearch,
            input: json!({ "query": "rust 2026" }),
        }
    );
    let HostedPart::Result {
        id,
        tool: ServerTool::WebSearch,
        outcome: HostedOutcome::Found(found),
    } = parts[1]
    else {
        panic!("search should answer with results: {:?}", parts[1]);
    };
    assert_eq!(id, "srvtoolu_1");
    assert_eq!(found[0].url(), "https://blog.rust-lang.org/");
    assert_eq!(found[0].title(), Some("Rust Blog"));
    assert_eq!(found[0].date(), Some("May 1, 2026"));

    let response = finished(&events).clone();
    assert_eq!(response.stop(), &Stop::Paused);
    let hosted: Vec<&Hosted> = response
        .content()
        .iter()
        .filter_map(Content::as_hosted)
        .collect();
    assert_eq!(hosted.len(), 2);
    let stored = serde_json::to_value(hosted[1]).expect("hosted block should serialize");
    let restored: Hosted = serde_json::from_value(stored).expect("hosted block should restore");
    assert_eq!(&restored, hosted[1]);

    let next = Server::streaming(&[]);
    let model = hosting(&next, &ServerTool::ALL);
    let continued = model
        .chat()
        .cache(Cache::Conversation)
        .server_tools(ServerTool::ALL)
        .user("what changed in rust this year?")
        .message(response.into_message())
        .build();
    let body = sent(&model, &continued, &next).await;

    let replayed = &body["messages"][1]["content"];
    assert_eq!(
        replayed[1],
        json!({
            "type": "server_tool_use",
            "id": "srvtoolu_1",
            "name": "web_search",
            "input": { "query": "rust 2026" },
        })
    );
    assert_eq!(replayed[2], result);
    assert!(replayed[3].get("cache_control").is_some());
}

#[tokio::test]
async fn hosted_blocks_stay_with_their_vendor() {
    let fetched = json!([{
        "type": "server_tool_use",
        "id": "srvtoolu_2",
        "name": "web_fetch",
        "input": { "url": "https://example.com/" },
    }, {
        "type": "web_fetch_tool_result",
        "tool_use_id": "srvtoolu_2",
        "content": { "type": "web_fetch_tool_result_error", "error_code": "url_not_accessible" },
    }]);
    let body = json!({
        "id": "msg_2",
        "model": "claude-opus-5-5",
        "content": fetched,
        "stop_reason": "end_turn",
        "usage": {},
    });
    let answer = Server::routed(move |_: &Exchange| Reply::json(200, &body));
    let model = hosting(&answer, &ServerTool::ALL);
    let response = model
        .chat()
        .user("read https://example.com/")
        .send()
        .await
        .expect("answer should decode");
    let parts: Vec<&HostedPart> = response
        .content()
        .iter()
        .filter_map(Content::as_hosted)
        .map(Hosted::part)
        .collect();
    assert_eq!(
        parts[1],
        &HostedPart::Result {
            id: "srvtoolu_2".to_owned(),
            tool: ServerTool::WebFetch,
            outcome: HostedOutcome::Failed("url_not_accessible".to_owned()),
        }
    );

    let elsewhere = Server::streaming(&[]);
    let model = elsewhere.completions("gpt-5", CompletionsDialect::Compatible);
    let request = model
        .chat()
        .user("read https://example.com/")
        .message(response.into_message())
        .user("and?")
        .build();
    let body = sent(&model, &request, &elsewhere).await;

    let roles: Vec<&Value> = body["messages"]
        .as_array()
        .expect("messages should be a list")
        .iter()
        .map(|message| &message["role"])
        .collect();
    assert_eq!(roles, [&json!("user"), &json!("user")]);
}

#[tokio::test]
async fn unanswered_server_call_is_dropped_before_new_turn() {
    let body = json!({
        "id": "msg_3",
        "model": "claude-opus-5-5",
        "content": [
            { "type": "text", "text": "Searching." },
            { "type": "server_tool_use", "id": "srvtoolu_3", "name": "web_search", "input": { "query": "rust" } },
        ],
        "stop_reason": "pause_turn",
        "usage": {},
    });
    let paused = Server::routed(move |_: &Exchange| Reply::json(200, &body));
    let model = hosting(&paused, &ServerTool::ALL);
    let response = model
        .chat()
        .user("what is new in rust?")
        .send()
        .await
        .expect("answer should decode");
    assert_eq!(response.stop(), &Stop::Paused);

    for (follow, blocks) in [(None, 2), (Some("never mind"), 1)] {
        let server = Server::streaming(&[]);
        let model = hosting(&server, &ServerTool::ALL);
        let mut chat = model
            .chat()
            .server_tools(ServerTool::ALL)
            .user("what is new in rust?")
            .message(response.message());
        if let Some(follow) = follow {
            chat = chat.user(follow);
        }
        let body = sent(&model, &chat.build(), &server).await;

        let replayed = body["messages"][1]["content"]
            .as_array()
            .expect("assistant turn should hold blocks");
        assert_eq!(replayed.len(), blocks, "{follow:?}");
        assert_eq!(replayed[0]["text"], json!("Searching."));
    }
}
