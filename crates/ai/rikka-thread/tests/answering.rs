use futures_util::StreamExt;
use rikka_agent::{CompactionPrompts, Function};
use rikka_llm::{
    Content, Effort, Image, ModelInfo, Reasoning, Stop, Tool, ToolUse, Usage, Value, json,
};
use rikka_llm_accounts::ConnectedModel;
use rikka_prompt::{Facts, Prompt};
use rikka_test_llm::{Asked, Scripted, Turn};
use rikka_thread::{
    Answer, Assistant, Desk, DeskEvent, QuestionEvent, Replies, Reply, Selection, Store, Thread,
};
use rikka_tools::{ASK, BASH, Toolbox, VIEW, facts};

use crate::support::{PNG, Watched, connected_model};

const AGENT: &str = r#"+++
name = "Agent"

[limits]
most = 8

[params]
tasks = "The agents to start, at most {{most}} of them."
task = "What the agent is to do, and what it is to report."
label = "A short name for the agent."
job = "The agent job to carry on with, rather than starting one."
model = "The model to run it on, by name."
effort = "How hard it is to think."
+++

Starts agents of its own and returns their job ids.
"#;

const REVISED: &str = "Your instructions changed; these replace the earlier ones.";

fn project() -> (tempfile::TempDir, Assistant) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    std::fs::write(folder.path().join("a.txt"), "hello\n").expect("file should be written");
    let assistant = assistant_in(folder.path());
    (folder, assistant)
}

fn assistant_in(project: &std::path::Path) -> Assistant {
    instructed(project, "You are answering in a test.")
}

fn instructed(project: &std::path::Path, instruction: &str) -> Assistant {
    let toolbox = Toolbox::builder(project)
        .build()
        .expect("tools should be declared");

    Assistant::builder(toolbox)
        .instruction(instruction)
        .revision_heading(REVISED)
        .compaction_prompts(
            CompactionPrompts::new("Say what has happened so far.")
                .framing("What happened so far: {{summary}}"),
        )
        .subagents(
            vec!["You are an agent of a test.".to_owned()],
            &agent(),
            &tool_facts(),
        )
        .build()
        .expect("prompts and tools should load")
}

fn agent() -> Prompt {
    Prompt::read("agent", AGENT).expect("document should parse")
}

fn sized(window: u64) -> ModelInfo {
    serde_json::from_value(json!({
        "id": "scripted",
        "name": "Scripted",
        "context": window,
        "output": 1_000,
        "vision": false,
        "efforts": [],
    }))
    .expect("selection should have model info")
}

fn tool_facts() -> Facts {
    facts().expect("shipped documents should parse")
}

fn tool(stem: &str) -> String {
    match stem {
        "agent" => agent().name().to_owned(),
        stem => tool_facts()
            .get(&format!("{stem}_tool"))
            .expect("every tool should have a name")
            .to_owned(),
    }
}

fn equipped(assistant: Assistant) -> Desk {
    let mut desk = Desk::new(Thread::new());
    let _unequipped = desk.equip(assistant);
    desk
}

fn assistant(desk: &Desk) -> &Assistant {
    desk.assistant().expect("desk should be equipped")
}

async fn answered(desk: &mut Desk, hired: ConnectedModel, ask: &str) -> Vec<Reply> {
    let replies = desk.ask(hired, ask).expect("agent should build");
    applied(desk, replies).await
}

async fn applied(desk: &mut Desk, mut replies: Replies) -> Vec<Reply> {
    let mut seen = Vec::new();
    while let Some(reply) = replies.next().await {
        desk.apply(&reply);
        seen.push(reply);
    }
    seen
}

fn request_text(request: &Asked) -> String {
    fn words(value: &Value, into: &mut Vec<String>) {
        match value {
            Value::String(text) => into.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| words(item, into)),
            Value::Object(fields) => fields.values().for_each(|field| words(field, into)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    words(&request.body()["messages"], &mut texts);
    texts.join("\n")
}

fn standing(request: &Asked) -> Vec<String> {
    request.body()["system"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn turn_reads_file_and_answers() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(&tool(BASH), json!({ "command": "cat a.txt" }))
        .says("It says hello.");

    let mut desk = equipped(assistant);
    let replies = answered(&mut desk, connected_model(&scripted), "what is in a.txt?").await;

    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");
    let ran = replies
        .iter()
        .find_map(|reply| match reply {
            Reply::Finished { result, .. } => Some((result.output.clone(), result.failed)),
            _ => None,
        })
        .expect("tool should run");
    assert!(ran.0.contains("hello"), "{ran:?}");
    assert!(!ran.1);

    let mut answer = Answer::new("scripted");
    for reply in &replies {
        answer.apply(reply);
    }
    let (text, rounds) = answer.finish();

    assert_eq!(text.text(), "It says hello.");
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].calls().len(), 1);
    assert_eq!(rounds[0].calls()[0].tool(), tool(BASH));

    drop(folder);
}

#[tokio::test]
async fn next_turn_sees_previous_results() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(&tool(BASH), json!({ "command": "cat a.txt" }))
        .says("It says hello.")
        .says("Still hello.");

    let mut desk = equipped(assistant);
    answered(&mut desk, connected_model(&scripted), "what is in a.txt?").await;
    let replies = answered(&mut desk, connected_model(&scripted), "and now?").await;
    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");

    let requests = scripted.requests();
    let transcript = requests
        .last()
        .expect("two requests should exist")
        .messages();
    assert_eq!(transcript.len(), 5, "{transcript:#?}");
    assert_eq!(transcript[0].text(), "what is in a.txt?");
    assert_eq!(transcript[1].tool_calls(), 1);
    assert_eq!(transcript[3].text(), "It says hello.");
    assert_eq!(transcript[4].text(), "and now?");

    drop(folder);
}

fn looking(project: &std::path::Path) -> Assistant {
    let declared = |name: &str, description: &str| {
        Tool::new(name)
            .description(description)
            .build()
            .expect("tool should be valid")
    };
    let looked = Function::new(declared("Look", "Looks at the screen"), |_: Value| async {
        Ok::<_, String>(
            rikka_agent::Reply::new("the screen")
                .image(Image::png(PNG))
                .load_tool("Point"),
        )
    });
    let pointed = Function::new(
        declared("Point", "Points at the screen"),
        |_: Value| async { Ok::<_, String>(rikka_agent::Reply::new("pointed")) },
    );
    let toolbox = Toolbox::builder(project)
        .tools([looked])
        .deferred([pointed])
        .build()
        .expect("tools should be declared");

    Assistant::builder(toolbox)
        .instruction("You are answering in a test.")
        .revision_heading(REVISED)
        .build()
        .expect("prompts and tools should load")
}

fn unmarked(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(name, _)| *name != "cache_control")
                .map(|(name, field)| (name.clone(), unmarked(field)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(unmarked).collect()),
        other => other.clone(),
    }
}

#[tokio::test]
async fn reopened_thread_resends_identical_request() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let assistant = looking(folder.path());
    let scripted = Scripted::new();
    scripted
        .asks("Look", json!({}))
        .says("Seen.")
        .says("Seen again.");

    let mut desk = equipped(assistant);
    answered(&mut desk, connected_model(&scripted), "look at the screen").await;

    let root = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(root.path());
    store.keep(desk.thread());
    drop(store);
    let thread = Store::new(root.path())
        .read(desk.thread().id())
        .expect("conversation should read back");
    let _kept = desk.open(thread);

    let replies = answered(&mut desk, connected_model(&scripted), "and now?").await;
    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");

    let requests = scripted.requests();
    assert_eq!(requests.len(), 3, "{requests:#?}");
    let (earlier, later) = (unmarked(requests[1].body()), unmarked(requests[2].body()));
    assert_eq!(later["system"], earlier["system"]);
    assert_eq!(later["tools"], earlier["tools"]);
    let sent = earlier["messages"]
        .as_array()
        .expect("request should have messages");
    let resent = later["messages"]
        .as_array()
        .expect("request should have messages");
    assert_eq!(resent[..sent.len()], sent[..]);
    assert!(
        sent[2].to_string().contains("tool_reference") && sent[2].to_string().contains("image"),
        "{:#}",
        sent[2]
    );
}

async fn answered_then_asked_again(
    scripted: &Scripted,
    prompt: &str,
) -> (rikka_thread::TurnText, Vec<rikka_thread::Round>) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let toolbox = Toolbox::builder(folder.path())
        .build()
        .expect("tools should be declared");
    let assistant = Assistant::builder(toolbox)
        .instruction("You are answering in a test.")
        .revision_heading(REVISED)
        .build()
        .expect("prompts and tools should load");
    let mut desk = equipped(assistant);
    let mut answer = Answer::new("scripted");
    for reply in answered(&mut desk, connected_model(scripted), prompt).await {
        answer.apply(&reply);
    }
    answered(&mut desk, connected_model(scripted), "and now?").await;
    answer.finish()
}

#[tokio::test]
async fn continued_answer_is_complete() {
    let scripted = Scripted::new();
    scripted
        .answers("The first half", Stop::MaxTokens)
        .says(" and the second.")
        .says("Done.");

    let (text, rounds) = answered_then_asked_again(&scripted, "write it").await;

    assert_eq!(text.text(), "The first half and the second.");
    assert!(rounds.is_empty(), "{rounds:#?}");
    let resent = unmarked(
        scripted
            .requests()
            .last()
            .expect("model should be asked again")
            .body(),
    );
    assert_eq!(
        resent["messages"][1]["content"],
        json!([
            { "type": "text", "text": "The first half" },
            { "type": "text", "text": " and the second." },
        ])
    );
}

#[tokio::test]
async fn web_search_is_shown_and_survives_reopening() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let toolbox = Toolbox::builder(folder.path())
        .web()
        .build()
        .expect("tools should be declared");
    let assistant = Assistant::builder(toolbox)
        .instruction("You are answering in a test.")
        .revision_heading(REVISED)
        .build()
        .expect("prompts and tools should load");
    let scripted = Scripted::new();
    scripted
        .searches(
            "rust 2026",
            &["https://blog.rust-lang.org/"],
            "Looking.",
            Stop::Paused,
        )
        .says("A new edition.")
        .says("Still new.")
        .says("Again.");

    let mut desk = equipped(assistant);
    let mut answer = Answer::new("scripted");
    for reply in answered(
        &mut desk,
        connected_model(&scripted),
        "what is new in rust?",
    )
    .await
    {
        answer.apply(&reply);
    }
    let (text, rounds) = answer.finish();
    assert_eq!(text.text(), "A new edition.");
    let searched = &rounds[0].calls()[0];
    assert!(searched.is_hosted());
    assert_eq!(searched.tool(), "web_search");
    assert_eq!(searched.input(), &json!({ "query": "rust 2026" }));
    assert_eq!(
        searched.output(),
        Some("https://blog.rust-lang.org/ — https://blog.rust-lang.org/")
    );
    assert_eq!(rounds[1].text(), "Looking.");

    answered(&mut desk, connected_model(&scripted), "and now?").await;
    let root = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(root.path());
    store.keep(desk.thread());
    drop(store);
    let thread = Store::new(root.path())
        .read(desk.thread().id())
        .expect("conversation should read back");
    let _kept = desk.open(thread);
    answered(&mut desk, connected_model(&scripted), "and again?").await;

    let requests = scripted.requests();
    assert_eq!(requests.len(), 4, "{requests:#?}");
    let (earlier, later) = (unmarked(requests[2].body()), unmarked(requests[3].body()));
    let sent = earlier["messages"]
        .as_array()
        .expect("request should have messages");
    let resent = later["messages"]
        .as_array()
        .expect("request should have messages");
    assert_eq!(resent[..sent.len()], sent[..]);
    assert_eq!(sent[1]["content"][0]["type"], json!("server_tool_use"));
    assert_eq!(
        sent[1]["content"][1]["type"],
        json!("web_search_tool_result")
    );

    drop(folder);
}

#[tokio::test]
async fn reasoning_is_replayed_next_turn() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .answers(
            [
                Content::Reasoning(Reasoning::new("look in a.txt")),
                Content::ToolUse(ToolUse::new(
                    "call_1",
                    tool(BASH),
                    json!({ "command": "cat a.txt" }),
                )),
            ],
            Stop::ToolUse,
        )
        .says([
            Content::Reasoning(Reasoning::new("that is all it says")),
            Content::text("It says hello."),
        ])
        .says("Still hello.");

    let mut desk = equipped(assistant);
    answered(&mut desk, connected_model(&scripted), "what is in a.txt?").await;
    answered(&mut desk, connected_model(&scripted), "and now?").await;

    let requests = scripted.requests();
    let read_back: Vec<String> = requests
        .last()
        .expect("thread should resume")
        .messages()
        .iter()
        .flat_map(Turn::thoughts)
        .collect();

    assert_eq!(read_back, ["look in a.txt", "that is all it says"]);

    drop(folder);
}

#[tokio::test]
async fn instructions_are_pinned_and_changes_noticed() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let monday = instructed(folder.path(), "Today is Monday.");
    let tuesday = instructed(folder.path(), "Today is Tuesday.");
    let scripted = Scripted::new();
    scripted.says("one").says("two").says("three");

    let mut desk = equipped(monday);
    answered(&mut desk, connected_model(&scripted), "first").await;
    let _monday = desk.equip(tuesday);
    answered(&mut desk, connected_model(&scripted), "second").await;
    answered(&mut desk, connected_model(&scripted), "third").await;

    let requests = scripted.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(standing(&requests[0])[0], "Today is Monday.");
    for request in &requests {
        assert_eq!(standing(request), standing(&requests[0]));
    }
    let read =
        |request: &Asked| -> Vec<String> { request.messages().iter().map(Turn::text).collect() };
    let second = read(&requests[1]);
    let third = read(&requests[2]);
    let revised = second.last().expect("second request should exist");
    assert!(revised.starts_with(REVISED), "{revised}");
    assert!(revised.contains("Today is Tuesday."), "{revised}");
    assert!(revised.ends_with("second"), "{revised}");
    assert_eq!(third[..second.len()], second[..]);
    assert_eq!(third.last().map(String::as_str), Some("third"));

    drop(folder);
}

#[tokio::test]
async fn screenshot_is_kept_with_its_call() {
    const PNG: [u8; 72] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7b,
        0x40, 0xe8, 0xdd, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8,
        0xcf, 0xc0, 0xc0, 0xf0, 0x9f, 0x01, 0x00, 0x07, 0xff, 0x01, 0xff, 0xb8, 0x04, 0x35, 0xe0,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let (folder, assistant) = project();
    std::fs::write(folder.path().join("shot.png"), PNG).expect("picture should be written");
    let scripted = Scripted::new();
    scripted
        .asks(&tool(VIEW), json!({ "path": "shot.png" }))
        .says("It is two pixels wide.");

    let replies = answered(
        &mut equipped(assistant),
        connected_model(&scripted),
        "look at it",
    )
    .await;

    let mut answer = Answer::new("a-model");
    for reply in &replies {
        answer.apply(reply);
    }
    let (_, rounds) = answer.finish();
    let displayed = rounds[0].calls()[0].displayed();

    assert_eq!(displayed.len(), 1, "{replies:?}");
    assert_eq!(displayed[0].data().as_ref(), PNG);
    assert_eq!(displayed[0].extent(), Some((2, 1)));

    drop(folder);
}

#[tokio::test]
async fn stopped_run_keeps_partial_answer() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted.says("never asked");

    let mut desk = equipped(assistant);
    let replies = desk
        .ask(connected_model(&scripted), "stop me")
        .expect("conversation should build");
    let unsent = desk.stop();
    assert!(unsent.is_empty(), "{unsent:?}");

    let replies = applied(&mut desk, replies).await;

    assert_eq!(replies.last(), Some(&Reply::Stopped));
    assert_eq!(
        scripted.remaining(),
        1,
        "nothing was ever asked of the model"
    );

    drop(folder);
}

#[tokio::test]
async fn full_context_compacts_and_continues() {
    let (folder, assistant) = project();

    let scripted = Scripted::new();
    scripted.costing(Usage {
        input: 17_000,
        ..Usage::default()
    });
    scripted
        .asks(&tool(BASH), json!({ "command": "cat a.txt" }))
        .says("They asked what is in a.txt and it says hello.")
        .says("It says hello.");

    let replies = answered(
        &mut equipped(assistant),
        ConnectedModel::new(scripted.model(), Some(sized(30_000)), Effort::default()),
        "what is in a.txt?",
    )
    .await;

    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");
    let folding = replies
        .iter()
        .position(|reply| matches!(reply, Reply::Compacting))
        .expect("compaction should be announced");
    let (folded, summary) = replies
        .iter()
        .enumerate()
        .find_map(|(at, reply)| match reply {
            Reply::Compacted(lead) => Some((at, lead.clone())),
            _ => None,
        })
        .expect("full context should compact");
    assert!(folding < folded);
    assert!(summary.contains("it says hello"), "{summary}");

    let requests = scripted.requests();
    assert_eq!(requests.len(), 3, "{requests:#?}");

    let summarising = requests[1].messages();
    assert_eq!(
        summarising.len(),
        3,
        "the fold shares what the turn had sent"
    );
    assert_eq!(requests[1].tools(), requests[0].tools());
    assert!(summarising[0].text().contains("what is in a.txt?"));
    assert!(
        summarising[2]
            .text()
            .contains("Say what has happened so far.")
    );

    let carried = requests[2].messages();
    assert_eq!(carried.len(), 1, "{carried:#?}");
    assert_eq!(
        carried[0].text(),
        summary,
        "the thread keeps the very lead the model carried on from"
    );
    assert!(
        !summary.trim_end().ends_with("it says hello."),
        "an automatic fold tells the model to carry on: {summary}"
    );

    drop(folder);
}

#[tokio::test]
async fn subagent_report_reaches_parent() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(
            &tool("agent"),
            json!({ "tasks": [{ "label": "a.txt", "task": "Say what a.txt holds." }] }),
        )
        .says("a.txt holds hello.")
        .says("a.txt holds hello.")
        .says("a.txt holds hello.");

    let mut desk = equipped(assistant);
    let replies = answered(&mut desk, connected_model(&scripted), "what is in a.txt?").await;
    let assistant = self::assistant(&desk);

    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");
    let started = replies
        .iter()
        .find_map(|reply| match reply {
            Reply::Finished { result, .. } => Some((result.output.clone(), result.failed)),
            _ => None,
        })
        .expect("agent should start");
    assert!(!started.1, "{started:?}");
    assert!(started.0.contains("label=\"a.txt\""), "{started:?}");
    assert!(started.0.contains("job=\"1\""), "{started:?}");
    assert!(!started.0.contains("holds hello"), "{started:?}");

    let mut replies_seen: Vec<String> = replies
        .iter()
        .filter_map(|reply| match reply {
            Reply::Noticed(notice) => Some(notice.clone()),
            _ => None,
        })
        .collect();
    if replies_seen.is_empty() {
        replies_seen = assistant
            .notices()
            .wait()
            .await
            .iter()
            .map(|notice| notice.text().to_owned())
            .collect();
    }
    assert_eq!(replies_seen.len(), 1, "{replies_seen:?}");
    assert!(
        replies_seen[0].contains("kind=\"agent\""),
        "{replies_seen:?}"
    );
    assert!(replies_seen[0].contains("reported"), "{replies_seen:?}");

    let job = assistant.jobs().snapshot();
    assert_eq!(job.len(), 1, "{job:?}");
    assert_eq!(job[0].state(), &rikka_thread::State::Reported);

    let requests = scripted.requests();
    assert!(
        requests[0].tools().contains(&tool("agent")) && requests[0].tools().contains(&tool(ASK)),
        "{:?}",
        requests[0].tools()
    );
    assert!(
        requests[1..].iter().any(|request| {
            let declared = request.tools();
            !declared.contains(&tool("agent"))
                && !declared.contains(&tool(ASK))
                && declared.contains(&tool(BASH))
        }),
        "the agent it started works with neither an agent tool nor a person to ask"
    );

    drop(folder);
}

#[tokio::test]
async fn interrupted_run_restores_from_journal() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(&tool(BASH), json!({ "command": "cat a.txt" }))
        .says("It says hello.")
        .says("Still hello.");

    let mut desk = equipped(assistant);
    let mut replies = desk
        .ask(connected_model(&scripted), "what is in a.txt?")
        .expect("agent should build");
    while let Some(reply) = replies.next().await {
        desk.apply(&reply);
        if matches!(reply, Reply::Finished { .. }) {
            break;
        }
    }
    drop(replies);

    let store = Store::new(folder.path());
    store.keep(desk.thread());
    drop(store);
    let cut_off = Store::new(folder.path())
        .read(desk.thread().id())
        .expect("journal should read back");
    let turns = cut_off.turns();
    assert_eq!(turns.len(), 2, "{turns:#?}");
    let body = turns[1].body();
    assert_eq!(body.rounds().len(), 1);
    let ran = &body.rounds()[0].calls()[0];
    assert_eq!(ran.tool(), tool(BASH));
    assert!(ran.output().is_some_and(|output| output.contains("hello")));

    let _interrupted = desk.open(cut_off);
    assert!(matches!(
        answered(&mut desk, connected_model(&scripted), "and now?")
            .await
            .last(),
        Some(Reply::Done | Reply::Failed(_))
    ));

    let requests = scripted.requests();
    let transcript = requests.last().expect("run should continue").messages();
    assert_eq!(transcript[0].text(), "what is in a.txt?");
    assert_eq!(transcript[1].tool_calls(), 1);
    assert_eq!(
        transcript
            .last()
            .expect("transcript should not be empty")
            .text(),
        "and now?"
    );

    drop(folder);
}

#[tokio::test]
async fn question_round_trips_to_user() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(
            &tool(ASK),
            json!({
                "questions": [{
                    "header": "Database",
                    "question": "Which database?",
                    "options": [
                        { "label": "Postgres", "description": "one the team already runs" },
                        { "label": "SQLite", "description": "one file, no server" },
                    ],
                }],
            }),
        )
        .says("They want SQLite.");

    let mut watched = Watched::new(assistant);
    let replies = watched
        .desk
        .ask(connected_model(&scripted), "ask them what they want")
        .expect("agent should build");
    let mut asked = 0;
    let replies = watched
        .respond(replies, |desk, event| {
            let DeskEvent::Asked(QuestionEvent::Posted(survey)) = event else {
                return;
            };
            asked += 1;
            assert_eq!(desk.questions().len(), 1);
            desk.answer_question(
                survey.id(),
                vec![Selection::new(vec!["SQLite".to_owned()], None)],
            );
        })
        .await;

    assert_eq!(asked, 1);
    assert!(watched.desk.questions().is_empty());

    assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");
    let requests = scripted
        .requests()
        .iter()
        .any(|request| request_text(request).contains("\"chosen\":[\"SQLite\"]"));
    assert!(requests, "selected answer missing from request");

    drop(folder);
}

#[tokio::test]
async fn stopping_conversation_leaves_its_agents_running() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks_all([
            ToolUse::new(
                "call_agent",
                tool("agent"),
                json!({ "tasks": [{ "label": "nap", "task": "Sleep a moment, then report." }] }),
            ),
            ToolUse::new("call_wait", tool(BASH), json!({ "command": "sleep 3" })),
        ])
        .asks(&tool(BASH), json!({ "command": "sleep 1" }))
        .says("Slept.");

    let mut desk = equipped(assistant);
    let mut replies = desk
        .ask(connected_model(&scripted), "start a napping agent")
        .expect("agent should build");
    while let Some(reply) = replies.next().await {
        desk.apply(&reply);
        if matches!(&reply, Reply::Finished { id, .. } if &**id == "call_agent") {
            let _unsent = desk.stop();
        }
    }
    assert!(!desk.is_answering());
    let assistant = self::assistant(&desk);

    let running = assistant.jobs().snapshot();
    assert_eq!(running.len(), 1, "{running:?}");
    assert!(running[0].state().is_running(), "{running:?}");

    let arrived = assistant.notices().wait().await;
    assert!(arrived[0].text().contains("reported"), "{arrived:?}");
    assert!(arrived[0].wakes(), "{arrived:?}");

    drop(folder);
}

#[tokio::test]
async fn kept_agent_resumes_after_reopening() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let kept = tempfile::tempdir().expect("temp dir should be created");
    std::fs::write(folder.path().join("a.txt"), "hello\n").expect("file should be written");
    let thread = Thread::new();
    let id = thread.id().to_owned();

    let first = Scripted::new();
    first
        .asks(
            &tool("agent"),
            json!({ "tasks": [{ "label": "a.txt", "task": "Say what a.txt holds." }] }),
        )
        .says("a.txt holds hello.")
        .says("a.txt holds hello.");
    {
        let mut desk = Desk::new(thread);
        desk.keep_in(Store::new(kept.path()));
        let _unequipped = desk.equip(assistant_in(folder.path()));
        answered(&mut desk, connected_model(&first), "what is in a.txt?").await;
        let _reported = assistant(&desk).notices().wait().await;
        desk.keep();
    }

    let again = Scripted::new();
    again
        .asks(
            &tool("agent"),
            json!({ "tasks": [{ "job": 1, "task": "Now say it backwards." }] }),
        )
        .says("olleh")
        .says("olleh");
    let store = Store::new(kept.path());
    let mut desk = Desk::new(store.read(&id).expect("thread should read back"));
    desk.keep_in(store);
    let _unequipped = desk.equip(assistant_in(folder.path()));

    let restored = assistant(&desk).jobs().snapshot();
    assert_eq!(restored.len(), 1, "{restored:?}");
    assert_eq!(restored[0].id(), 1);
    assert_eq!(restored[0].label(), "a.txt");
    assert_eq!(restored[0].state(), &rikka_thread::State::Reported);

    let resumed = answered(&mut desk, connected_model(&again), "and backwards?").await;
    let assistant = assistant(&desk);
    assert!(
        resumed.iter().any(|reply| matches!(
            reply,
            Reply::Finished { result, .. } if result.output.contains("doing=\"resumed\"")
        )),
        "{resumed:?}"
    );
    let _reported = assistant.notices().wait().await;
    assert_eq!(
        assistant.jobs().snapshot()[0].state(),
        &rikka_thread::State::Reported
    );

    let briefed = "You are an agent of a test.";
    let carried = again
        .requests()
        .into_iter()
        .find(|request| standing(request).first().map(String::as_str) == Some(briefed))
        .expect("the agent should be asked again");
    let said = request_text(&carried);
    assert!(said.contains("Say what a.txt holds."), "{said}");
    assert!(said.contains("a.txt holds hello."), "{said}");
    assert!(said.contains("Now say it backwards."), "{said}");
}
