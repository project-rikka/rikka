use rikka_agent::{
    Agent, Compaction, CompactionPrompts, Content, Error, Function, Message, Role, Tool,
    ToolResult, ToolUse, Value, json,
};
use rikka_test_llm::Scripted;

fn compaction_prompts() -> CompactionPrompts {
    CompactionPrompts::new("summarise the conversation")
        .framing("what follows is a summary")
        .focus_heading("the person wants kept:")
        .resume_prompt("carry straight on")
}

fn compaction() -> Compaction {
    Compaction::new(30_000, compaction_prompts())
        .output(4_000)
        .keep(0)
}

fn history() -> Vec<Message> {
    vec![Message::user("older"), Message::assistant("older still")]
}

#[tokio::test]
async fn full_thread_compacts_before_send() {
    let script = Scripted::new();
    script.says("what happened before").says("the answer");

    let agent = Agent::new(script.model())
        .compaction(compaction())
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent
        .resume(history())
        .filled(14_000)
        .send("and now?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "the answer");
    let requests = script.requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    let shared = requests[0].messages();
    assert_eq!(shared.len(), 3);
    assert!(shared[2].text().contains("and now?"));
    assert!(shared[2].text().contains("summarise the conversation"));

    let messages = requests[1].messages();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].text().starts_with("what follows is a summary"));
    assert!(messages[0].text().contains("what happened before"));
    assert!(messages[0].text().ends_with("carry straight on"));
}

#[tokio::test]
async fn overflow_error_compacts_and_retries() {
    let script = Scripted::new();
    script
        .fails(rikka_llm::Error::ContextOverflow)
        .says("what happened before")
        .says("the answer");

    let agent = Agent::new(script.model())
        .compaction(Compaction::new(0, compaction_prompts()).keep(0))
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent
        .resume(history())
        .send("and now?")
        .await
        .expect("run should finish");

    assert_eq!(outcome.text(), "the answer");
    let requests = script.requests();
    assert_eq!(requests.len(), 3, "{requests:#?}");
    assert_eq!(
        requests[1].messages().len(),
        1,
        "what overflowed is retold, not resent"
    );
    assert!(
        requests[2].messages()[0]
            .text()
            .contains("what happened before")
    );
}

#[tokio::test]
async fn overflow_after_compaction_is_error() {
    let script = Scripted::new();
    script
        .fails(rikka_llm::Error::ContextOverflow)
        .says("what happened before")
        .fails(rikka_llm::Error::ContextOverflow);

    let agent = Agent::new(script.model())
        .compaction(compaction())
        .build()
        .expect("agent configuration should be valid");

    let refused = agent.resume(history()).send("and now?").await;

    assert!(matches!(
        refused,
        Err(Error::Model(rikka_llm::Error::ContextOverflow))
    ));
}

#[tokio::test]
async fn persistent_tool_state_follows_summary() {
    let script = Scripted::new();
    script.says("what happened before");

    let skill = Tool::new("Skill")
        .description("Load a skill")
        .string("skill", "Which")
        .build()
        .expect("tool should be valid");
    let lasting = Function::new(skill, |_: Value| async {
        Ok::<String, String>(String::new())
    })
    .survives_compaction();
    let agent = Agent::new(script.model())
        .function(lasting)
        .compaction(compaction().restatement(|| Some("job 3 is running".to_owned())))
        .build()
        .expect("agent configuration should be valid");

    let history = vec![
        Message::user("review it"),
        Message::new(
            Role::Assistant,
            vec![Content::ToolUse(ToolUse::new(
                "1",
                "Skill",
                json!({ "skill": "review" }),
            ))],
        ),
        Message::tool_results([ToolResult::new("1", "how to review")]),
        Message::assistant("reviewed"),
    ];
    let mut session = agent.resume(history);
    session
        .compact("")
        .await
        .expect("compaction should succeed");

    let lead = session.history()[0].text();
    assert!(lead.contains("<recalled tool=\"Skill\">\nhow to review\n</recalled>"));
    assert!(lead.ends_with("job 3 is running"));
}

#[tokio::test]
async fn tools_loaded_before_summary_are_relisted() {
    let script = Scripted::new();
    script.says("what happened before");

    let agent = Agent::new(script.model())
        .compaction(compaction())
        .build()
        .expect("agent configuration should be valid");

    let history = vec![
        Message::user("weather?"),
        Message::new(
            Role::Assistant,
            vec![Content::ToolUse(ToolUse::new(
                "1",
                "ToolSearch",
                json!({ "query": "weather" }),
            ))],
        ),
        Message::tool_results([ToolResult::new("1", "loaded")
            .reference("get_weather")
            .reference("get_forecast")]),
        Message::assistant("28C"),
    ];
    let mut session = agent.resume(history);
    session
        .compact("")
        .await
        .expect("compaction should succeed");

    let lead = session.history()[0].text();
    assert!(lead.ends_with(": get_forecast, get_weather"), "{lead}");
}
