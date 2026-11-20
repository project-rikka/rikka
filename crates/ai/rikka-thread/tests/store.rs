use rikka_sync::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_llm::{Effort, Media, Usage};
use rikka_llm_accounts::{Backend, Selection};
use rikka_thread::{Ask, Attachment, Round, Store, Thread};

use crate::support::PNG;

fn going(text: &str) -> Thread {
    let mut thread = Thread::new();
    thread.push_ask(text);
    thread
}

fn read_back(folder: &Path, thread: &Thread) -> Thread {
    Store::new(folder)
        .read(thread.id())
        .expect("conversation should read back")
}

fn journal(folder: &Path, thread: &Thread) -> PathBuf {
    folder.join(thread.id()).join("turns.jsonl")
}

fn files_under(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder).expect("directory should be readable") {
        let path = entry.expect("directory entry should be readable").path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn selection(backend: &str, model: &str, effort: Effort) -> Selection {
    Selection::new(
        Backend::from_id(backend).expect("backend should be registered"),
        model,
        effort,
    )
}

#[test]
fn saved_thread_round_trips_and_lists_summary() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let mut older = going("the first one");
    older.answer("done", Vec::new());
    std::thread::sleep(std::time::Duration::from_millis(2));
    let newer = going("the second one");
    store.keep(&older);
    store.keep(&newer);
    drop(store);

    assert_eq!(read_back(folder.path(), &older), older);
    let listed = Store::new(folder.path()).summaries();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id(), newer.id(), "the one changed last leads");
    assert_eq!(listed[1].title(), "the first one");
    assert_eq!(listed[1].updated(), older.updated());
}

#[test]
fn conversation_settings_survive_reopen() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let mut first = Thread::new();
    first.push_ask(
        Ask::new("/one")
            .with_expansion("the whole of one")
            .with_context(vec!["<apps>Animate</apps>".to_owned()]),
    );
    first.select(selection("anthropic", "opus", Effort::High));
    first.compact("what happened");
    first.record_usage(Usage {
        input: 100,
        output: 20,
        cache_read: 900,
        ..Usage::default()
    });
    let mut second = going("two");
    second.select(selection("openai", "gpt", Effort::Low));
    store.keep(&first);
    store.keep(&second);
    drop(store);

    let store = Store::new(folder.path());
    let read = store.read(first.id()).expect("first thread should open");
    assert_eq!(
        read.selection(),
        Some(&selection("anthropic", "opus", Effort::High))
    );
    assert_eq!(read.context_tokens(), 1_020);
    assert_eq!(read.transcript("a-model").len(), 1);
    assert_eq!(read.turns()[0].body().context(), ["<apps>Animate</apps>"]);
    assert_eq!(read.turns()[0].body().text(), "/one");
    assert_eq!(read.turns()[0].body().sent_text(), "the whole of one");
    assert_eq!(
        store
            .read(second.id())
            .expect("second thread should open")
            .selection()
            .expect("thread should keep its selection")
            .model(),
        "gpt"
    );
}

#[test]
fn streaming_answer_saves_latest_snapshot() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let mut thread = going("go");
    for step in 0..48 {
        thread.answer_partial(format!("{step}: {}", "thinking ".repeat(512)), Vec::new());
        store.keep(&thread);
    }
    drop(store);

    let read = read_back(folder.path(), &thread);
    assert_eq!(read.turns().len(), 2);
    assert!(read.turns()[1].body().text().starts_with("47: "));
    let length = std::fs::metadata(journal(folder.path(), &thread))
        .expect("journal should exist")
        .len();
    assert!(
        length < 48 * 4608 / 2,
        "the journal kept every answer that was still arriving: {length} bytes"
    );

    let store = Store::new(folder.path());
    let mut nothing = going("go");
    nothing.answer_partial("thinking", Vec::new());
    store.keep(&nothing);
    nothing.answer("   ", Vec::new());
    store.keep(&nothing);
    drop(store);
    assert_eq!(
        read_back(folder.path(), &nothing).turns().len(),
        1,
        "an answer that came to nothing leaves no turn behind"
    );
}

#[test]
fn attachments_are_stored_once_beside_journal() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let shot =
        || Attachment::new("shot.png", Media::Png, PNG).expect("view should return the picture");
    let round: Round = serde_json::from_str(
        r#"{"said":"","calls":[{"id":"call_1","tool":"View","input":{"path":"shot.png"},"output":"shot.png"}]}"#,
    )
    .expect("round should parse");

    let mut thread = Thread::new();
    thread.push_ask(Ask::new("what is this?").with_attachments(vec![shot()]));
    thread.answer("a picture", vec![round]);
    thread.push_ask(Ask::new("and this?").with_attachments(vec![shot()]));
    store.keep(&thread);
    drop(store);

    let journal =
        std::fs::read_to_string(journal(folder.path(), &thread)).expect("journal should exist");
    assert!(
        !journal.contains("iVBOR"),
        "an attachment's bytes are not written into the journal"
    );
    let pictures = files_under(&folder.path().join(thread.id()))
        .into_iter()
        .filter(|path| std::fs::read(path).is_ok_and(|bytes| bytes == PNG))
        .count();
    assert_eq!(pictures, 1, "the same picture is written down once");

    let read = read_back(folder.path(), &thread);
    let first = read.turns()[0].body();
    let attached = first.attachments();
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].data().as_ref(), PNG);
    assert_eq!(read.turns()[2].body().attachments()[0].name(), "shot.png");
    assert_eq!(
        read.turns()[1].body().rounds()[0].calls()[0].output(),
        Some("shot.png")
    );
}

#[test]
fn truncated_journal_line_is_skipped() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let mut thread = going("go");
    thread.answer("done", Vec::new());
    store.keep(&thread);
    drop(store);

    let at = journal(folder.path(), &thread);
    let mut contents = std::fs::read_to_string(&at).expect("journal should exist");
    contents.push_str("{\"turn\":{\"speaker\":\"mod");
    std::fs::write(&at, contents).expect("file should be written");

    let read = read_back(folder.path(), &thread);
    assert_eq!(read.turns().len(), 2, "what was whole is still there");
    assert_eq!(read.turns()[1].body().text(), "done");
}

#[test]
fn reopened_thread_saves_without_loss() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let round: Round = serde_json::from_str(
        r#"{"said":"","calls":[{"id":"call_1","tool":"Bash","input":{"command":"ls"},"output":"ok"}]}"#,
    )
    .expect("round should parse");
    let mut thread = going("go");
    thread.answer_partial("half of it", Vec::new());
    store.keep(&thread);
    thread.answer("all of it", vec![round]);
    store.keep(&thread);
    drop(store);

    let read = read_back(folder.path(), &thread);
    let other = Store::new(folder.path());
    other.keep(&read);
    drop(other);

    let again = read_back(folder.path(), &thread);
    assert_eq!(again.turns().len(), 2);
    assert_eq!(again.turns()[1].body().text(), "all of it");
    assert_eq!(
        again.turns()[1].body().rounds()[0].calls()[0].output(),
        Some("ok")
    );
}

#[test]
fn discard_removes_files_and_reports_failure() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let store = Store::new(folder.path());
    let thread = going("go");
    store.keep(&thread);
    store.discard(thread.id());
    store.discard(thread.id());
    drop(store);
    assert!(Store::new(folder.path()).summaries().is_empty());
    assert!(!folder.path().join(thread.id()).exists());

    let nowhere = folder.path().join("nowhere");
    std::fs::write(&nowhere, "not a folder").expect("file should be written");
    let store = Store::new(&nowhere);
    let trouble = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&trouble);
    store.watch(move |error| {
        recorder.lock().push(error.to_string());
    });
    store.keep(&going("go"));
    drop(store);

    let recorder = trouble.lock();
    assert_eq!(recorder.len(), 1, "{recorder:?}");
}
