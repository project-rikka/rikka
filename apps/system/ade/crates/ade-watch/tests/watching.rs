use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use ade_watch::{Sink, Watch};

const QUIET: Duration = Duration::from_millis(80);

const WRITE_INTERVAL: Duration = Duration::from_millis(20);

const PATIENCE: Duration = Duration::from_secs(10);

const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

fn watching(folders: &[&PathBuf]) -> BTreeSet<PathBuf> {
    folders.iter().map(|folder| (*folder).clone()).collect()
}

fn reporting() -> (Sink, Receiver<BTreeSet<PathBuf>>) {
    let (sender, receiver) = mpsc::channel();
    let sink: Sink = Arc::new(move |folders| {
        sender.send(folders).unwrap_or(());
    });
    (sink, receiver)
}

fn await_watching(receiver: &Receiver<BTreeSet<PathBuf>>, folder: &Path) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        std::fs::write(folder.join(".probe"), "probe").expect("failed to write probe");
        match receiver.recv_timeout(PROBE_TIMEOUT) {
            Ok(reported) if reported.contains(folder) => return,
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => panic!("watcher stopped"),
        }
    }
    panic!("{} is not watched", folder.display());
}

#[test]
fn burst_of_writes_is_one_report() {
    let project = tempfile::tempdir().expect("failed to create temp dir");
    let root = project.path().to_owned();
    let src = root.join("src");
    std::fs::create_dir(&src).expect("failed to create folder");

    let (sink, receiver) = reporting();
    let mut watch = Watch::new(QUIET, sink);
    watch.set_folders(watching(&[&root, &src]));
    await_watching(&receiver, &src);

    for index in 0..50 {
        std::fs::write(src.join(format!("file{index}.rs")), "fn main() {}")
            .expect("failed to write file");
    }
    std::fs::write(root.join("Cargo.toml"), "[package]").expect("failed to write file");

    let reported = receiver
        .recv_timeout(PATIENCE)
        .expect("writes not reported");
    assert!(reported.contains(&src), "{reported:?}");
    assert!(reported.len() <= 2, "{reported:?}");
}

#[test]
fn unwatched_folder_is_not_reported() {
    let project = tempfile::tempdir().expect("failed to create temp dir");
    let root = project.path().to_owned();
    let left = root.join("left");
    let right = root.join("right");
    std::fs::create_dir(&left).expect("failed to create folder");
    std::fs::create_dir(&right).expect("failed to create folder");

    let (sink, receiver) = reporting();
    let mut watch = Watch::new(QUIET, sink);
    watch.set_folders(watching(&[&left, &right]));
    await_watching(&receiver, &left);

    watch.set_folders(watching(&[&right]));
    await_watching(&receiver, &right);

    std::fs::write(left.join("ignored.txt"), "unwatched").expect("failed to write file");
    std::fs::write(right.join("seen.txt"), "watched").expect("failed to write file");

    let reported = receiver
        .recv_timeout(PATIENCE)
        .expect("writes not reported");
    assert_eq!(reported, watching(&[&right]));
}

#[test]
fn continuous_writes_still_report() {
    let project = tempfile::tempdir().expect("failed to create temp dir");
    let root = project.path().to_owned();

    let (sink, receiver) = reporting();
    let mut watch = Watch::new(QUIET, sink);
    watch.set_folders(watching(&[&root]));
    await_watching(&receiver, &root);

    let target = root.join("busy.log");
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    let writer = std::thread::spawn(move || {
        while !stopping.load(Ordering::Relaxed) {
            std::fs::write(&target, "more").expect("failed to write file");
            std::thread::sleep(WRITE_INTERVAL);
        }
    });

    let reported = receiver
        .recv_timeout(PATIENCE)
        .expect("continuous writes not reported");
    stop.store(true, Ordering::Relaxed);
    writer.join().expect("writer thread panicked");

    assert_eq!(reported, watching(&[&root]));
}
