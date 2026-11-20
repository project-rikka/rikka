use rikka_llm::json;
use rikka_test_llm::{Asked, Scripted};
use rikka_thread::{Ask, Speaker, Store, Thread};
use rikka_tools::{BASH, WRITE};

use crate::support::{LEAD, Watched, connected_model, project, tool};

fn shape(request: &Asked) -> Vec<(String, String, usize)> {
    request
        .messages()
        .iter()
        .map(|turn| (turn.role().to_owned(), turn.text(), turn.tool_calls()))
        .collect()
}

#[tokio::test]
async fn interjection_is_separate_turn() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted
        .asks(&tool(BASH), json!({ "command": "cat a.txt" }))
        .says("It says hello, and b.txt is missing.")
        .says("You are welcome.");

    let mut watched = Watched::new(assistant);
    let replies = watched
        .desk
        .ask(connected_model(&scripted), "read a.txt")
        .expect("agent should build");
    watched
        .desk
        .interject(Ask::new("and b.txt too"))
        .expect("running conversation should accept a message");
    assert_eq!(watched.desk.queued().len(), 1);

    watched.drive(replies).await;

    assert!(watched.desk.queued().is_empty());
    let thread = watched.desk.thread();
    let speakers: Vec<Speaker> = thread.turns().iter().map(|turn| turn.speaker()).collect();
    assert_eq!(
        speakers,
        [Speaker::User, Speaker::Model, Speaker::User, Speaker::Model]
    );
    assert_eq!(thread.turns()[2].body().text(), "and b.txt too");
    assert_eq!(
        thread.turns()[3].body().text(),
        "It says hello, and b.txt is missing."
    );

    let live = scripted.requests();
    let sent = live.last().expect("model should be asked again").clone();
    assert_eq!(
        sent.messages().last().map(|turn| turn.text()),
        Some(format!("{LEAD}\nand b.txt too"))
    );

    let again = watched
        .desk
        .ask(connected_model(&scripted), "thanks")
        .expect("agent should build");
    watched.drive(again).await;
    let replayed = shape(
        scripted
            .requests()
            .last()
            .expect("model should be asked again"),
    );
    assert_eq!(replayed[..shape(&sent).len()], shape(&sent));

    drop(folder);
}

#[tokio::test]
async fn stop_returns_unsent_messages() {
    let (folder, assistant) = project();
    let scripted = Scripted::new();
    scripted.says("never asked");

    let mut watched = Watched::new(assistant);
    let replies = watched
        .desk
        .ask(connected_model(&scripted), "go")
        .expect("agent should build");
    watched
        .desk
        .interject(Ask::new("one more thing"))
        .expect("running conversation should accept a message");

    assert_eq!(watched.desk.stop(), [Ask::new("one more thing")]);
    assert!(watched.desk.interject(Ask::new("too late")).is_err());
    watched.drive(replies).await;
    assert!(watched.desk.queued().is_empty());
    assert!(!watched.desk.is_answering());
    assert_eq!(watched.desk.thread().turns().len(), 1);

    drop(folder);
}

#[tokio::test]
async fn rewind_drops_later_turns_and_restores_files() {
    let (folder, assistant) = project();
    let threads = tempfile::tempdir().expect("temp dir should be created");
    let scripted = Scripted::new();
    scripted
        .asks(
            &tool(WRITE),
            json!({ "path": "a.txt", "content": "changed\n" }),
        )
        .says("Rewrote it.");

    let store = Store::new(threads.path());
    let mut watched = Watched::new(assistant);
    watched.desk.keep_in(store.clone());
    let replies = watched
        .desk
        .ask(connected_model(&scripted), "change a.txt")
        .expect("agent should build");
    watched.drive(replies).await;

    let path = folder.path().join("a.txt");
    assert_eq!(
        std::fs::read_to_string(&path).expect("file should be readable"),
        "changed\n"
    );
    let history = store.history(watched.desk.thread().id());
    let turn = &watched.desk.thread().turns()[0];
    let (stamp, at) = (turn.stamp(), turn.at());
    let changed = history.changed_since(at).expect("history should read");
    assert_eq!(changed.len(), 1);
    assert!(changed[0].ends_with("a.txt"));

    assert!(watched.desk.thread_mut().rewind(stamp));
    let thread: &Thread = watched.desk.thread();
    assert!(thread.turns().is_empty());
    assert!(thread.title().is_empty());
    let rewound = history.rewind(at).expect("history should read");
    assert_eq!(rewound.restored.len(), 1);
    assert!(rewound.failed.is_empty());
    assert_eq!(
        std::fs::read_to_string(&path).expect("file should be readable"),
        "hello\n"
    );

    watched.desk.keep();
    let id = watched.desk.thread().id().to_owned();
    drop(watched);
    drop(store);
    let read = Store::new(threads.path())
        .read(&id)
        .expect("conversation should read back");
    assert!(read.turns().is_empty());
    assert!(
        Store::new(threads.path())
            .history(&id)
            .changed_since(0)
            .expect("history should read")
            .is_empty()
    );
}
