use std::sync::{Arc, Mutex};

use rikka_agent::{
    Agent, Content, Event, Interjection, Interjections, Message, Role, Tool, Value, json,
};
use rikka_test_llm::Scripted;

fn weather() -> Tool {
    Tool::new("get_weather")
        .description("Get current weather")
        .string("location", "City name")
        .build()
        .expect("tool should be valid")
}

fn heard_ids(interjected: &Arc<Mutex<Vec<u64>>>) -> impl Fn(Event) + Send + Sync + 'static {
    let interjected = Arc::clone(interjected);
    move |event: Event| {
        if let Event::Interjected { id } = event {
            interjected
                .lock()
                .expect("mutex should not be poisoned")
                .push(id);
        }
    }
}

fn is_user_message(message: &Message) -> bool {
    message.role() == Role::User
        && message
            .content()
            .iter()
            .all(|content| matches!(content, Content::Text(_)))
}

#[tokio::test]
async fn interjection_follows_tool_results() {
    let script = Scripted::new();
    script
        .asks("get_weather", json!({ "location": "Tokyo" }))
        .says("Tokyo is 28C, and Osaka next.");

    let interjections = Interjections::new();
    let posting = interjections.clone();
    let interjected = Arc::new(Mutex::new(Vec::new()));
    let agent = Agent::new(script.model())
        .tool(weather(), move |_: Value| {
            let posting = posting.clone();
            async move {
                posting.post(Interjection::new(7, vec![Content::text("also Osaka")]));
                Ok::<_, String>("28C".to_owned())
            }
        })
        .interjections(interjections.clone())
        .observer(heard_ids(&interjected))
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent
        .run("weather in Tokyo?")
        .await
        .expect("run should finish");

    let messages = outcome.messages();
    assert_eq!(messages.len(), 5, "{messages:#?}");
    assert!(matches!(
        messages[2].content().first(),
        Some(Content::ToolResult(_))
    ));
    assert!(is_user_message(&messages[3]));
    assert_eq!(messages[3].text(), "also Osaka");
    assert_eq!(messages[4].role(), Role::Assistant);
    assert_eq!(
        *interjected.lock().expect("mutex should not be poisoned"),
        vec![7]
    );
    assert!(interjections.take().is_empty());
    assert_eq!(script.requests().len(), 2);
}

#[tokio::test]
async fn pending_message_starts_new_turn() {
    let script = Scripted::new();
    script.says("First.").says("Second.");

    let interjections = Interjections::new();
    interjections.post(Interjection::new(3, vec![Content::text("and then?")]));
    let interjected = Arc::new(Mutex::new(Vec::new()));
    let agent = Agent::new(script.model())
        .interjections(interjections.clone())
        .observer(heard_ids(&interjected))
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("go").await.expect("run should finish");

    assert_eq!(outcome.text(), "Second.");
    assert_eq!(outcome.turns(), 2);
    let messages = outcome.messages();
    assert_eq!(messages[1].text(), "First.");
    assert!(is_user_message(&messages[2]));
    assert_eq!(messages[2].text(), "and then?");
    assert_eq!(
        *interjected.lock().expect("mutex should not be poisoned"),
        vec![3]
    );
}

#[tokio::test]
async fn withdrawn_message_is_not_sent() {
    let script = Scripted::new();
    script.says("Only this.");

    let interjections = Interjections::new();
    interjections.post(Interjection::new(1, vec![Content::text("never mind")]));
    assert!(interjections.withdraw(1));
    assert!(!interjections.withdraw(1));

    let agent = Agent::new(script.model())
        .interjections(interjections)
        .build()
        .expect("agent configuration should be valid");
    let outcome = agent.run("go").await.expect("run should finish");

    assert_eq!(outcome.turns(), 1);
    assert_eq!(outcome.messages().len(), 2);
}
