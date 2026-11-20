use std::path::Path;

use rikka_nix_journal::{
    Error, Journal, KEPT_RUNS, LINE_BYTES, PAGE_BYTES, TRANSCRIPT_BYTES, Written,
};
use rikka_nix_protocol::{Action, Operation, PageRecord, RunId, RunState};

const APPLY: Operation = Operation::Apply {
    action: Action::Switch,
};

fn opened(folder: &Path) -> Journal {
    let opened = Journal::open(folder).expect("the folder opens");
    assert!(opened.rejected.is_empty(), "{:?}", opened.rejected);
    opened.journal
}

fn done(generation: u32) -> RunState {
    RunState::Done {
        generation: Some(generation),
    }
}

#[test]
fn transcript_pages_back_what_was_written() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let mut journal = opened(folder.path());
    let (run, mut transcript) = journal.begin(APPLY).expect("a run begins");

    let written: Vec<Written> = ["building", "", "hello> world"]
        .into_iter()
        .map(|line| {
            transcript
                .append(line)
                .expect("it writes")
                .expect("there is room")
        })
        .collect();
    assert_eq!(
        written.iter().map(|line| line.offset).collect::<Vec<_>>(),
        [0, 9, 10]
    );

    assert_eq!(
        journal.page(run.id, 0).expect("it reads"),
        PageRecord {
            lines: vec![
                "building".to_owned(),
                String::new(),
                "hello> world".to_owned()
            ],
            next: 23,
            end: true,
        }
    );
    assert_eq!(
        journal
            .page(run.id, written[2].offset)
            .expect("it reads")
            .lines,
        ["hello> world"]
    );
    assert!(matches!(
        journal.page(run.id, 3),
        Err(Error::Offset { offset: 3, .. })
    ));
    assert!(matches!(
        journal.page(RunId::new(1), 0),
        Err(Error::UnknownRun(_))
    ));
}

#[test]
fn written_lines_are_single_and_capped() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let mut journal = opened(folder.path());
    let (run, mut transcript) = journal.begin(APPLY).expect("a run begins");

    let long = transcript
        .append(&"é".repeat(LINE_BYTES))
        .expect("it writes")
        .expect("there is room");
    assert!(long.text.len() <= LINE_BYTES);
    assert!(long.text.ends_with('…'));
    let split = transcript
        .append("first\nsecond\r")
        .expect("it writes")
        .expect("there is room");
    assert_eq!(split.text, "first second ");

    assert_eq!(
        journal.page(run.id, 0).expect("it reads").lines,
        [long.text, split.text]
    );
}

#[test]
fn large_transcripts_come_in_pages() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let mut journal = opened(folder.path());
    let (run, mut transcript) = journal.begin(APPLY).expect("a run begins");
    let line = "x".repeat(999);
    let count = (PAGE_BYTES * 5 / 2 / 1000) as usize;
    for _ in 0..count {
        transcript.append(&line).expect("it writes");
    }

    let mut lines = Vec::new();
    let mut pages = 0;
    let mut from = 0;
    loop {
        let page = journal.page(run.id, from).expect("it reads");
        pages += 1;
        lines.extend(page.lines);
        from = page.next;
        if page.end {
            break;
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(lines.len(), count);
    assert!(lines.iter().all(|read| *read == line));
}

#[test]
fn transcript_stops_at_its_limit() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let mut journal = opened(folder.path());
    let (_, mut transcript) = journal.begin(APPLY).expect("a run begins");
    let line = "y".repeat(LINE_BYTES - 1);
    let mut kept = 0u64;
    while let Some(written) = transcript.append(&line).expect("it writes") {
        kept = written.offset + written.text.len() as u64 + 1;
    }
    assert!(kept <= TRANSCRIPT_BYTES);
    assert!(kept > TRANSCRIPT_BYTES - LINE_BYTES as u64 * 2);
    assert_eq!(transcript.append("later").expect("it writes"), None);
    let size = std::fs::metadata(transcript.path())
        .expect("the transcript exists")
        .len();
    assert_eq!(size, kept);
}

#[test]
fn reopening_marks_unfinished_runs_interrupted() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let (finished, unfinished) = {
        let mut journal = opened(folder.path());
        let (finished, _) = journal.begin(APPLY).expect("a run begins");
        journal.finish(finished.id, done(42)).expect("it finishes");
        let (unfinished, _) = journal.begin(APPLY).expect("a run begins");
        (finished.id, unfinished.id)
    };

    let journal = opened(folder.path());
    let runs: Vec<_> = journal.runs().collect();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, unfinished);
    assert_eq!(runs[0].state, RunState::Interrupted);
    assert!(runs[0].finished.is_some());
    assert_eq!(runs[1].id, finished);
    assert_eq!(runs[1].state, done(42));
}

#[test]
fn only_the_newest_runs_are_kept() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    let mut journal = opened(folder.path());
    let mut ids = Vec::new();
    for generation in 0..KEPT_RUNS + 2 {
        let (run, _) = journal.begin(APPLY).expect("a run begins");
        journal
            .finish(run.id, done(generation as u32))
            .expect("it finishes");
        ids.push(run.id);
    }
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));

    let kept: Vec<RunId> = journal.runs().rev().map(|run| run.id).collect();
    assert_eq!(kept, ids[2..]);
    let files = std::fs::read_dir(folder.path())
        .expect("the folder lists")
        .count();
    assert_eq!(files, KEPT_RUNS * 2);
    assert_eq!(opened(folder.path()).runs().count(), KEPT_RUNS);
}

#[test]
fn malformed_record_is_reported_not_fatal() {
    let folder = tempfile::tempdir().expect("a scratch folder");
    std::fs::write(folder.path().join("5.json"), "garbage").expect("it writes");
    std::fs::write(folder.path().join("5.log"), "kept\n").expect("it writes");
    std::fs::write(folder.path().join("7.log"), "orphan\n").expect("it writes");
    let opened = Journal::open(folder.path()).expect("the folder opens");
    assert!(matches!(opened.rejected[..], [Error::Record { .. }]));
    assert_eq!(opened.journal.runs().count(), 0);
    assert!(folder.path().join("5.log").exists());
    assert!(!folder.path().join("7.log").exists());
}
