use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rikka_agent::{
    Agent, Budget, CancelToken, Content, Event, HostedOutcome, HostedPart, Limit, Message, Role,
    ServerTool, Stop, Tool, ToolResult, ToolUse, Usage, Value, json,
};
use rikka_test_llm::Scripted;

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn tool_result(last: &Message) -> ToolResult {
    match &last.content()[0] {
        Content::ToolResult(result) => result.clone(),
        other => panic!("the outstanding call went unanswered: {other:?}"),
    }
}

fn weather() -> Tool {
    Tool::new("get_weather")
        .description("Get current weather")
        .string("location", "City name")
        .build()
        .expect("tool should be valid")
}

async fn answer_weather(input: Value) -> Result<String, String> {
    let city = input
        .get("location")
        .and_then(Value::as_str)
        .ok_or("no location")?;
    Ok(format!("{city}: 28C"))
}

#[tokio::test]
async fn toolless_question_is_one_turn() {
    let script = Scripted::new();
    script.says("Hello");

    let agent = Agent::new(script.model())
        .build()
        .expect("agent configuration should be valid");
    let outcome = agent.run("hi").await.expect("run should finish");

    assert_eq!(outcome.text(), "Hello");
    assert_eq!(outcome.turns(), 1);
    assert!(outcome.is_complete());
    assert_eq!(outcome.messages().len(), 2);
}

#[tokio::test]
async fn tool_call_runs_and_loop_continues() {
    let script = Scripted::new();
    script
        .asks("get_weather", json!({ "location": "Tokyo" }))
        .says("It is 28C in Tokyo.");

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent
        .run("weather in Tokyo?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "It is 28C in Tokyo.");
    assert_eq!(outcome.turns(), 2);

    let requests = script.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].messages().len(), 3);
    assert_eq!(requests[1].tools().len(), 1);
}

#[tokio::test]
async fn tool_failure_is_reported_to_model() {
    let script = Scripted::new();
    script
        .asks("get_weather", json!({}))
        .says("I need a city name.");

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("weather?").await.expect("run should finish");

    assert_eq!(outcome.text(), "I need a city name.");
    assert_eq!(outcome.turns(), 2);
}

#[tokio::test]
async fn unknown_tool_gets_error_result() {
    let script = Scripted::new();
    script
        .asks("delete_everything", json!({}))
        .says("I cannot.");

    let agent = Agent::new(script.model())
        .build()
        .expect("agent configuration should be valid");
    let outcome = agent.run("go on then").await.expect("run should finish");

    assert_eq!(outcome.text(), "I cannot.");
    let blocks = script.requests()[1].messages()[2].blocks();
    assert_eq!(blocks, 1);
}

#[tokio::test]
async fn parallel_calls_answered_in_one_message() {
    let script = Scripted::new();
    script
        .asks_all([
            ToolUse::new("a", "get_weather", json!({ "location": "Tokyo" })),
            ToolUse::new("b", "get_weather", json!({ "location": "Osaka" })),
        ])
        .says("Both are warm.");

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .build()
        .expect("agent configuration should be valid");

    agent.run("compare them").await.expect("run should finish");

    let results = script.requests()[1].messages()[2].blocks();
    assert_eq!(results, 2);
}

#[tokio::test]
async fn turn_budget_stops_loop() {
    let script = Scripted::new();
    for _ in 0..10 {
        script.asks("get_weather", json!({ "location": "Tokyo" }));
    }

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .budget(Budget::new().turns(3))
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("loop forever").await.expect("run should finish");

    assert_eq!(outcome.turns(), 3);
    assert_eq!(outcome.exhausted(), Some(Limit::Turns));
    assert!(!outcome.is_complete());
}

#[tokio::test]
async fn token_budget_stops_run() {
    let script = Scripted::new();
    script.costing(Usage {
        input: 100,
        output: 100,
        ..Usage::default()
    });
    for _ in 0..10 {
        script.asks("get_weather", json!({ "location": "Tokyo" }));
    }

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .budget(Budget::new().turns(100).tokens(500))
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("loop forever").await.expect("run should finish");

    assert_eq!(outcome.exhausted(), Some(Limit::Tokens));
    assert!(outcome.usage().total() >= 500);
}

#[tokio::test]
async fn subagent_cost_counts_toward_parent() {
    let below = Scripted::new();
    below.costing(Usage {
        input: 10,
        output: 5,
        ..Usage::default()
    });
    below.says("Tokyo is 28C.");

    let researcher = Agent::new(below.model())
        .name("researcher")
        .description("Looks things up")
        .build()
        .expect("subagent configuration should be valid");

    let above = Scripted::new();
    above.costing(Usage {
        input: 20,
        output: 10,
        ..Usage::default()
    });
    above
        .asks("researcher", json!({ "task": "weather in Tokyo" }))
        .says("It is 28C.");

    let agent = Agent::new(above.model())
        .subagent(researcher)
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent
        .run("ask the researcher")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "It is 28C.");
    assert_eq!(outcome.usage().total(), 30 + 30 + 15);
    assert_eq!(below.remaining(), 0);
}

#[tokio::test]
async fn rejects_duplicate_tool_names() {
    let script = Scripted::new();

    let error = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .tool(weather(), answer_weather)
        .build()
        .expect_err("the model could not tell them apart");

    assert!(matches!(error, rikka_agent::Error::DuplicateTool(name) if name == "get_weather"));
}

#[tokio::test]
async fn cancelled_run_leaves_valid_transcript() {
    let script = Scripted::new();
    script.asks("get_weather", json!({ "location": "Tokyo" }));

    let cancel = CancelToken::new();
    let stop = cancel.clone();

    let agent = Agent::new(script.model())
        .tool(weather(), answer_weather)
        .cancel_token(cancel)
        .observer(move |event| {
            if matches!(event, Event::TurnFinished { .. }) {
                stop.cancel();
            }
        })
        .build()
        .expect("agent configuration should be valid");

    let mut chat = agent.session();
    let error = chat
        .send("weather?")
        .await
        .expect_err("the run was stopped");
    assert!(matches!(error, rikka_agent::Error::Cancelled));

    let last = chat
        .history()
        .last()
        .expect("transcript should not be empty");
    assert_eq!(last.role(), Role::User);

    let answered = tool_result(last);
    assert_eq!(answered.id(), "call_1");
    assert!(answered.is_failed());
}

#[tokio::test]
async fn stop_cancels_running_tool() {
    let script = Scripted::new();
    script.asks("get_weather", json!({ "location": "Tokyo" }));

    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let dropped = Arc::new(AtomicBool::new(false));
    let marked = Arc::clone(&dropped);
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);

    let agent = Agent::new(script.model())
        .tool(weather(), move |_| {
            let marked = Arc::clone(&marked);
            async move {
                let _drop_flag = DropFlag(marked);
                std::future::pending::<Result<String, String>>().await
            }
        })
        .cancel_token(cancel)
        .observer(move |event| {
            if matches!(event, Event::ToolStarted { .. }) {
                stop.cancel();
            }
            sink.lock().expect("log should be writable").push(event);
        })
        .build()
        .expect("agent configuration should be valid");

    let mut chat = agent.session();
    let error = tokio::time::timeout(Duration::from_secs(5), chat.send("weather?"))
        .await
        .expect("stopped run should not wait for the tool")
        .expect_err("the run was stopped");

    assert!(matches!(error, rikka_agent::Error::Cancelled));
    assert!(
        dropped.load(Ordering::SeqCst),
        "running tool not dropped on cancel"
    );
    assert!(
        events
            .lock()
            .expect("log should be readable")
            .iter()
            .any(|event| matches!(event, Event::ToolFinished { failed: true, .. })),
        "no ToolFinished event after cancel"
    );

    let answered = tool_result(
        chat.history()
            .last()
            .expect("transcript should not be empty"),
    );
    assert_eq!(answered.id(), "call_1");
    assert!(answered.is_failed());
}

#[tokio::test]
async fn truncated_answer_is_continued() {
    let script = Scripted::new();
    script
        .answers("the first half", Stop::MaxTokens)
        .says(" and the rest");

    let agent = Agent::new(script.model())
        .build()
        .expect("agent configuration should be valid");
    let outcome = agent
        .run("write it all out")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "the first half and the rest");
    assert_eq!(outcome.turns(), 2);
    assert!(outcome.is_complete());

    let requests = script.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]
            .messages()
            .last()
            .map(|turn| turn.role().to_owned()),
        Some("user".to_owned()),
        "the model is nudged to carry on"
    );
}

#[tokio::test]
async fn paused_answer_resumes_without_user_turn() {
    let script = Scripted::new();
    script
        .searches(
            "rust 2026",
            &["https://blog.rust-lang.org/"],
            "Looking.",
            Stop::Paused,
        )
        .says("Rust shipped a new edition.");
    let hosted = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&hosted);

    let agent = Agent::new(script.model())
        .server_tools(ServerTool::ALL)
        .observer(move |event| {
            if let Event::Hosted(part) = event {
                heard.lock().expect("hosted parts").push(part);
            }
        })
        .build()
        .expect("agent configuration should be valid");
    let outcome = agent
        .run("what is new in rust?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "Rust shipped a new edition.");
    assert_eq!(outcome.turns(), 2);
    let requests = script.requests();
    assert_eq!(
        requests[1]
            .messages()
            .last()
            .map(|turn| (turn.role().to_owned(), turn.blocks())),
        Some(("assistant".to_owned(), 3)),
        "the paused answer is sent back as it came"
    );
    let parts = hosted.lock().expect("hosted parts");
    assert!(matches!(
        parts.as_slice(),
        [
            HostedPart::Call { tool: ServerTool::WebSearch, .. },
            HostedPart::Result { outcome: HostedOutcome::Found(found), .. },
        ] if found[0].url() == "https://blog.rust-lang.org/"
    ));
}

#[tokio::test]
async fn client_tool_may_not_shadow_server_tool() {
    let script = Scripted::new();
    let shadow = Tool::new("web_search")
        .description("Search")
        .string("query", "What to find")
        .build()
        .expect("tool should be valid");

    let error = Agent::new(script.model())
        .server_tools([ServerTool::WebSearch])
        .tool(shadow, answer_weather)
        .build()
        .expect_err("the model could not tell them apart");

    assert!(matches!(error, rikka_agent::Error::DuplicateTool(name) if name == "web_search"));
}
