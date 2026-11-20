use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use rikka_compositor_reactor::{Interest, Reactor};

#[derive(Default)]
struct Log {
    entries: Vec<String>,
    done: bool,
}

#[test]
fn timers_fire_in_deadline_order() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let start = Instant::now();
    reactor.timer(start + Duration::from_millis(20), |log: &mut Log| {
        log.entries.push("late".into());
        log.done = true;
    });
    reactor.timer(start + Duration::from_millis(5), |log: &mut Log| {
        log.entries.push("early".into())
    });
    reactor.run(&mut log, |log| log.done).unwrap();
    assert_eq!(log.entries, ["early", "late"]);
    assert!(start.elapsed() >= Duration::from_millis(20));
}

#[test]
fn mailbox_delivers_cross_thread_messages() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let sender = reactor
        .mailbox(|log: &mut Log, message: u32| {
            log.entries.push(message.to_string());
            log.done = message == 2;
        })
        .unwrap();
    let worker = std::thread::spawn(move || {
        for message in 0..3 {
            sender.send(message).unwrap();
        }
    });
    reactor.run(&mut log, |log| log.done).unwrap();
    worker.join().unwrap();
    assert_eq!(log.entries, ["0", "1", "2"]);
}

#[test]
fn watch_reports_readable_and_removes_in_callback() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer.write_all(b"x").unwrap();
    let handle = reactor.clone();
    let token = std::rc::Rc::new(std::cell::Cell::new(None));
    let inside = token.clone();
    let watched = reactor
        .watch(
            reader.as_fd(),
            Interest::READABLE,
            move |log: &mut Log, ready| {
                assert!(ready.readable);
                log.entries.push("readable".into());
                log.done = true;
                handle.remove(inside.get().unwrap());
            },
        )
        .unwrap();
    token.set(Some(watched));
    reactor.run(&mut log, |log| log.done).unwrap();
    reactor
        .turn(&mut log, Some(Duration::from_millis(5)))
        .unwrap();
    assert_eq!(log.entries, ["readable"]);
}

#[test]
fn stale_token_ignores_reused_slot() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let stale = reactor.after(Duration::from_secs(60), |log: &mut Log| {
        log.entries.push("stale".into())
    });
    reactor.remove(stale);
    reactor.after(Duration::from_millis(5), |log: &mut Log| {
        log.entries.push("fresh".into());
        log.done = true;
    });
    reactor.remove(stale);
    reactor.run(&mut log, |log| log.done).unwrap();
    assert_eq!(log.entries, ["fresh"]);
}

#[test]
fn woken_task_runs_once_per_wake() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let task = reactor.task(|log: &mut Log| log.entries.push("task".into()));
    reactor.turn(&mut log, Some(Duration::ZERO)).unwrap();
    assert!(log.entries.is_empty());
    reactor.wake(task);
    reactor.wake(task);
    let start = Instant::now();
    reactor
        .turn(&mut log, Some(Duration::from_secs(5)))
        .unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(log.entries, ["task"]);
    reactor.remove(task);
    reactor.wake(task);
    reactor.turn(&mut log, Some(Duration::ZERO)).unwrap();
    assert_eq!(log.entries, ["task"]);
}

#[test]
fn cancelled_timers_keep_live_order() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    let start = Instant::now();
    for round in 0..500u64 {
        let token = reactor.timer(
            start + Duration::from_millis(1 + round % 7),
            |log: &mut Log| log.entries.push("cancelled".into()),
        );
        reactor.remove(token);
    }
    reactor.timer(start + Duration::from_millis(8), |log: &mut Log| {
        log.entries.push("second".into());
        log.done = true;
    });
    reactor.timer(start + Duration::from_millis(3), |log: &mut Log| {
        log.entries.push("first".into())
    });
    reactor.run(&mut log, |log| log.done).unwrap();
    assert_eq!(log.entries, ["first", "second"]);
}

#[test]
fn settle_runs_after_every_callback() {
    let reactor = Reactor::<Log>::new().unwrap();
    let mut log = Log::default();
    reactor.after_each(|log: &mut Log| {
        if log.entries.last().is_some_and(|last| last != "settled") {
            log.entries.push("settled".into());
        }
    });
    let (mut writer, reader) = UnixStream::pair().unwrap();
    writer.write_all(b"x").unwrap();
    let handle = reactor.clone();
    let token = std::rc::Rc::new(std::cell::Cell::new(None));
    let inside = token.clone();
    let watched = reactor
        .watch(
            reader.as_fd(),
            Interest::READABLE,
            move |log: &mut Log, _| {
                log.entries.push("readable".into());
                handle.remove(inside.get().unwrap());
            },
        )
        .unwrap();
    token.set(Some(watched));
    reactor.idle(|log: &mut Log| log.entries.push("idle".into()));
    let task = reactor.task(|log: &mut Log| log.entries.push("task".into()));
    reactor.wake(task);
    reactor.each_turn(|log: &mut Log| {
        log.entries.push("turn".into());
        log.done = true;
    });
    reactor.run(&mut log, |log| log.done).unwrap();
    assert_eq!(
        log.entries,
        [
            "readable", "settled", "idle", "settled", "task", "settled", "turn", "settled"
        ]
    );
}
