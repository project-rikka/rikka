use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_llm::wire::{Messages, Transport, endpoint, headers};
use rikka_llm::{Cache, Message, Model, Request, Stop, Tool, Ttl};
use rikka_test_llm::{Exchange, Reply, Server};
use serde_json::{Value, json};

use crate::support::answer;

const BETA: &str = "message-threads-2026-08-12";

fn model(server: &Server) -> Model {
    let transport = Transport::new(
        endpoint(server.url()).expect("test server URL should parse"),
        headers().build().expect("headers should build"),
    )
    .expect("transport should build");
    Messages::new(transport, "claude-opus-5-5")
        .betas(|_, _| "effort-2025-11-24".to_owned())
        .threaded()
        .build()
}

fn search() -> Tool {
    Tool::new("search")
        .description("Search the project")
        .string("query", "What to find")
        .build()
        .expect("tool should be valid")
}

fn opening(model: &Model, system: &str) -> Request {
    model
        .chat()
        .system(system)
        .tool(search())
        .user("hello")
        .cache(Cache::Conversation)
        .ttl(Ttl::Long)
        .cache_key("conversation-1")
        .build()
}

fn events(frames: &[Value]) -> Reply {
    let frames: Vec<String> = frames.iter().map(Value::to_string).collect();
    Reply::events(&frames.iter().map(String::as_str).collect::<Vec<_>>())
}

fn answered(id: &str) -> Reply {
    events(&[
        json!({ "type": "message_start", "message": { "id": id, "model": "claude-opus-5-5", "usage": { "input_tokens": 10 } } }),
        json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
        json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "hi there" } }),
        json!({ "type": "content_block_stop", "index": 0 }),
        json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 3 } }),
        json!({ "type": "message_stop" }),
    ])
}

fn paused(id: &str) -> Reply {
    events(&[
        json!({ "type": "message_start", "message": { "id": id, "model": "claude-opus-5-5", "usage": { "input_tokens": 10 } } }),
        json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "server_tool_use", "id": "srvtoolu_1", "name": "web_search", "input": {} } }),
        json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": "{\"query\":\"rust\"}" } }),
        json!({ "type": "content_block_stop", "index": 0 }),
        json!({ "type": "message_delta", "delta": { "stop_reason": "pause_turn" }, "usage": { "output_tokens": 3 } }),
        json!({ "type": "message_stop" }),
    ])
}

fn refused(status: u16, code: &str) -> Reply {
    Reply::json(
        status,
        &json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "thread refused",
                "details": { "error_code": code },
            },
        }),
    )
}

fn replying(answer: impl Fn(usize, &Value) -> Reply + Send + Sync + 'static) -> Server {
    let count = Arc::new(AtomicUsize::new(0));
    Server::routed(move |exchange: &Exchange| {
        let index = count.fetch_add(1, Ordering::SeqCst);
        answer(index, exchange.body().unwrap_or(&Value::Null))
    })
}

fn identified(index: usize, _: &Value) -> Reply {
    answered(&format!("msg_{index}"))
}

async fn conversed(model: &Model, request: &Request) -> Request {
    let response = answer(model, request).await.expect("turn should answer");
    let mut next = request.clone();
    next.push(response.into_message());
    next.push(Message::user("and then?"));
    next
}

fn thread_of(body: &Value) -> &Value {
    &body["thread"]
}

#[tokio::test]
async fn later_turn_sends_only_new_messages() {
    let server = replying(identified);
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    answer(&model, &second)
        .await
        .expect("second turn should answer");

    let exchanges = server.exchanges();
    let create = exchanges[0].body().expect("create carries a body");
    let next = exchanges[1].body().expect("continue carries a body");

    assert_eq!(thread_of(create), &json!({ "type": "create" }));
    assert_eq!(create["system"][0]["text"], json!("be brief"));
    assert_eq!(create["tools"][0]["name"], json!("search"));
    assert_eq!(
        create["cache_control"],
        json!({ "type": "ephemeral", "ttl": "1h" })
    );
    assert!(
        create["messages"][0]["content"][0]
            .get("cache_control")
            .is_none()
    );

    assert_eq!(
        thread_of(next),
        &json!({ "type": "continue", "previous_message_id": "msg_0" })
    );
    assert!(next.get("system").is_none());
    assert!(next.get("tools").is_none());
    assert_eq!(next["cache_control"], create["cache_control"]);
    assert_eq!(next["tool_choice"], create["tool_choice"]);
    assert_eq!(
        next["messages"],
        json!([{ "role": "user", "content": [{ "type": "text", "text": "and then?" }] }])
    );

    for exchange in &exchanges {
        assert_eq!(
            exchange.header("anthropic-beta"),
            Some(format!("effort-2025-11-24,{BETA}").as_str())
        );
    }
}

#[tokio::test]
async fn lapsed_thread_replays_whole_conversation() {
    let table = [
        ("thread_not_found", 404),
        ("thread_already_continued", 400),
        ("thread_continue_in_flight", 400),
        ("thread_fingerprint_mismatch", 400),
    ];
    for (code, status) in table {
        let server = replying(move |index, body| match thread_of(body)["type"].as_str() {
            Some("continue") => refused(status, code),
            _ => answered(&format!("msg_{index}")),
        });
        let model = model(&server);

        let second = conversed(&model, &opening(&model, "be brief")).await;
        let response = answer(&model, &second).await.expect("replay should answer");

        let bodies = server.requests();
        assert_eq!(bodies.len(), 3, "{code}");
        assert_eq!(
            thread_of(&bodies[2]),
            &json!({ "type": "create" }),
            "{code}"
        );
        assert_eq!(
            bodies[2]["messages"].as_array().map(Vec::len),
            Some(3),
            "{code}"
        );
        assert_eq!(bodies[2]["system"], bodies[0]["system"], "{code}");
        assert_eq!(response.id(), "msg_2", "{code}");
    }
}

#[tokio::test]
async fn failed_continue_keeps_thread() {
    let server = replying(|index, _| match index {
        1 => refused(401, "authentication_error"),
        _ => answered(&format!("msg_{index}")),
    });
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    assert!(answer(&model, &second).await.is_err());
    answer(&model, &second).await.expect("retry should answer");

    assert_eq!(
        thread_of(&server.requests()[2]),
        &json!({ "type": "continue", "previous_message_id": "msg_0" })
    );
}

#[tokio::test]
async fn replay_continues_from_its_own_answer() {
    let server = replying(
        |index, body| match (index, thread_of(body)["type"].as_str()) {
            (1, Some("continue")) => refused(404, "thread_not_found"),
            _ => answered(&format!("msg_{index}")),
        },
    );
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    let third = conversed(&model, &second).await;
    answer(&model, &third)
        .await
        .expect("third turn should answer");

    let bodies = server.requests();
    assert_eq!(
        thread_of(&bodies[3]),
        &json!({ "type": "continue", "previous_message_id": "msg_2" })
    );
    assert_eq!(bodies[3]["messages"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn unstorable_conversation_goes_unthreaded() {
    let server = replying(|index, body| match thread_of(body)["type"].as_str() {
        Some("continue") => refused(404, "thread_not_found"),
        _ => answered(&format!("msg_{index}")),
    });
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    let third = conversed(&model, &second).await;
    let fourth = conversed(&model, &third).await;
    answer(&model, &fourth)
        .await
        .expect("fourth turn should answer");

    let threads: Vec<Option<String>> = server
        .requests()
        .iter()
        .map(|body| thread_of(body)["type"].as_str().map(str::to_owned))
        .collect();
    let named = |kind: &str| Some(kind.to_owned());
    assert_eq!(
        threads,
        [
            named("create"),
            named("continue"),
            named("create"),
            named("continue"),
            None,
            None,
        ]
    );
}

#[tokio::test]
async fn unsupported_thread_sends_without_one() {
    let server = replying(|index, body| match thread_of(body)["type"].as_str() {
        Some(_) => refused(400, "thread_unsupported_request"),
        None => answered(&format!("msg_{index}")),
    });
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    answer(&model, &second)
        .await
        .expect("second turn should answer");

    let exchanges = server.exchanges();
    assert_eq!(exchanges.len(), 3);
    let fallback = exchanges[1].body().expect("fallback carries a body");
    assert!(fallback.get("thread").is_none());
    assert_eq!(fallback["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        exchanges[1].header("anthropic-beta"),
        Some("effort-2025-11-24")
    );
    let later = exchanges[2].body().expect("later turn carries a body");
    assert!(later.get("thread").is_none());
    assert_eq!(later["messages"].as_array().map(Vec::len), Some(3));
}

#[tokio::test]
async fn changed_instructions_start_new_thread() {
    let server = replying(identified);
    let model = model(&server);

    let mut second = conversed(&model, &opening(&model, "be brief")).await;
    let mut revised = opening(&model, "be thorough");
    revised.set_messages(second.messages().to_vec());
    second = revised;
    answer(&model, &second)
        .await
        .expect("second turn should answer");

    let bodies = server.requests();
    assert_eq!(thread_of(&bodies[1]), &json!({ "type": "create" }));
    assert_eq!(bodies[1]["system"][0]["text"], json!("be thorough"));
    assert_eq!(bodies[1]["messages"].as_array().map(Vec::len), Some(3));
}

#[tokio::test]
async fn rewritten_history_starts_new_thread() {
    let server = replying(identified);
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    let mut folded = second.clone();
    folded.set_messages(vec![Message::user("summary"), Message::user("and then?")]);
    answer(&model, &folded)
        .await
        .expect("folded turn should answer");

    let bodies = server.requests();
    assert_eq!(thread_of(&bodies[1]), &json!({ "type": "create" }));
    assert_eq!(bodies[1]["messages"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn refused_turn_is_not_continued() {
    let server = replying(|index, _| match index {
        0 => events(&[
            json!({ "type": "message_start", "message": { "id": "msg_0", "model": "claude-opus-5-5" } }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "refusal" } }),
            json!({ "type": "message_stop" }),
        ]),
        _ => answered(&format!("msg_{index}")),
    });
    let model = model(&server);

    let second = conversed(&model, &opening(&model, "be brief")).await;
    answer(&model, &second)
        .await
        .expect("second turn should answer");

    assert_eq!(
        thread_of(&server.requests()[1]),
        &json!({ "type": "create" })
    );
}

#[tokio::test]
async fn conversation_without_key_is_unthreaded() {
    let server = replying(identified);
    let model = model(&server);
    let request = model
        .chat()
        .system("be brief")
        .user("hello")
        .cache(Cache::Conversation)
        .build();

    answer(&model, &request).await.expect("turn should answer");

    let exchange = &server.exchanges()[0];
    let body = exchange.body().expect("request carries a body");
    assert!(body.get("thread").is_none());
    assert!(body.get("cache_control").is_none());
    assert!(
        body["messages"][0]["content"][0]
            .get("cache_control")
            .is_some()
    );
    assert_eq!(exchange.header("anthropic-beta"), Some("effort-2025-11-24"));
}

#[tokio::test]
async fn paused_turn_continues_with_no_new_messages() {
    let server = replying(|index, _| match index {
        0 => paused("msg_0"),
        _ => answered("msg_1"),
    });
    let model = model(&server);
    let opening = opening(&model, "be brief");

    let response = answer(&model, &opening).await.expect("turn should pause");
    assert_eq!(response.stop(), &Stop::Paused);
    let mut resumed = opening.clone();
    resumed.push(response.into_message());
    answer(&model, &resumed)
        .await
        .expect("paused turn should resume");

    let exchanges = server.exchanges();
    let next = exchanges[1].body().expect("continue carries a body");
    assert_eq!(
        thread_of(next),
        &json!({ "type": "continue", "previous_message_id": "msg_0" })
    );
    assert_eq!(next["messages"], json!([]));
}
