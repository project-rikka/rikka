use rikka_llm::{Role, Usage};
use rikka_thread::{Ask, Round, Speaker, Thread};

#[test]
fn first_message_names_thread() {
    let mut thread = Thread::new();
    assert!(thread.title().is_empty());
    thread.push_ask("Add the fonts to the selector\nand keep the old ones");
    thread.push_ask("now the other one");

    assert_eq!(thread.title(), "Add the fonts to the selector");
    assert_ne!(thread.id(), Thread::new().id());
}

#[test]
fn answer_is_journaled_while_streaming() {
    let mut thread = Thread::new();
    thread.push_ask("go");
    thread.answer_partial("look", Vec::new());

    assert_eq!(thread.turns().len(), 1);
    assert!(thread.is_running());
    assert_eq!(thread.all_turns().len(), 2);
    assert_eq!(thread.all_turns()[1].body().text(), "look");

    thread.answer_partial("looking", Vec::new());
    thread.answer("looking, and it is done", Vec::new());
    let transcript = thread.transcript("a-model");
    assert_eq!(thread.turns().len(), 2);
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].role(), Role::User);
    assert_eq!(transcript[0].text(), "go");
    assert_eq!(transcript[1].text(), "looking, and it is done");

    thread.answer("   ", Vec::new());
    assert_eq!(
        thread.turns().len(),
        2,
        "an answer that never arrived is no turn"
    );
}

#[test]
fn transcript_starts_at_latest_summary() {
    let mut thread = Thread::new();
    thread.push_ask("what is in a.rs?");
    thread.answer_partial("halfway", Vec::new());
    thread.compact("they asked about a.rs");
    assert_eq!(thread.turns()[1].speaker(), Speaker::Summary);

    thread.answer("on it", Vec::new());
    thread.compact("the second");
    thread.record_usage(Usage {
        input: 100,
        output: 20,
        cache_read: 900,
        ..Usage::default()
    });
    thread.push_ask("and b.rs?");

    let transcript = thread.transcript("a-model");
    assert_eq!(transcript.len(), 1);
    let text = transcript[0].text();
    assert!(text.contains("the second"), "{text}");
    assert!(text.contains("and b.rs?"), "{text}");
    assert!(!text.contains("about a.rs"), "{text}");
    assert_eq!(thread.context_tokens(), 1_020);
    assert_eq!(thread.summary().context_tokens(), 1_020);
}

#[test]
fn partially_compacted_turn_reads_from_summary() {
    let round: Round = serde_json::from_str(
        r#"{"said":"looking","calls":[{"id":"1","tool":"Bash","input":{},"output":"a"}]}"#,
    )
    .expect("round should parse");
    let mut thread = Thread::new();
    thread.push_ask("go");
    thread.answer("", vec![round]);
    thread.compact("what happened up to there");
    thread.answer("done", Vec::new());

    assert_eq!(thread.turns().len(), 4);
    let transcript = thread.transcript("a-model");
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].text(), "what happened up to there");
    assert_eq!(transcript[1].text(), "done");
}

#[test]
fn notices_restore_at_original_position() {
    let looked: Round = serde_json::from_str(
        r#"{"said":"looking","calls":[{"id":"1","tool":"Bash","input":{},"output":"a"}],"notices":["job 1 ended","job 2 ended"]}"#,
    )
    .expect("round should parse");
    let closing: Round = serde_json::from_str(r#"{"said":"done","notices":["job 3 ended"]}"#)
        .expect("round should parse");
    let mut thread = Thread::new();
    thread.push_ask("go");
    thread.answer("all done", vec![looked, closing]);

    let transcript = thread.transcript("a-model");
    let roles: Vec<Role> = transcript.iter().map(|message| message.role()).collect();
    assert_eq!(
        roles,
        [
            Role::User,
            Role::Assistant,
            Role::User,
            Role::Assistant,
            Role::User,
            Role::Assistant
        ]
    );
    assert_eq!(transcript[2].content().len(), 2);
    assert_eq!(transcript[2].text(), "job 1 ended\njob 2 ended");
    assert_eq!(transcript[4].text(), "job 3 ended");
}

#[test]
fn command_is_shown_typed_and_sent_expanded() {
    let mut thread = Thread::new();
    thread.push_ask(Ask::new("/simplify src").with_expansion("Review src for reuse."));

    assert_eq!(thread.title(), "/simplify src");
    assert_eq!(thread.turns()[0].body().text(), "/simplify src");
    assert_eq!(
        thread.transcript("a-model")[0].text(),
        "Review src for reuse."
    );
}

#[test]
fn join_expands_command_only_among_others() {
    let command = || Ask::new("/simplify ui").with_expansion("Review ui for reuse.");
    let cases = [
        (
            vec![command(), Ask::new(" ")],
            "/simplify ui",
            "Review ui for reuse.",
        ),
        (
            vec![command(), Ask::new("and keep it short")],
            "Review ui for reuse.\n\nand keep it short",
            "Review ui for reuse.\n\nand keep it short",
        ),
    ];

    for (asks, drafted, sent) in cases {
        let joined = Ask::join(asks).expect("asks should join");
        assert_eq!(joined.sent_text(), sent);
        assert_eq!(joined.into_draft().0, drafted);
    }
    assert_eq!(Ask::join([Ask::new("")]), None);
}
