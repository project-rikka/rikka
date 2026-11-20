use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rikka_compositor::Extent;
use rikka_seam::Request;
use rikka_session::desktop::{CAPABILITY, STATUS, Status};

use crate::common::Harness;

const EXTENT: Extent = Extent {
    width: 64,
    height: 64,
};
const PATIENCE: Duration = Duration::from_secs(30);
const RECHECK: Duration = Duration::from_millis(50);
const LAST_WORDS: &str = "fake shell is going down";

const CRASHES_ONCE: &str = r#"#!/bin/sh
echo "fake shell is going down" >&2
if [ -e "$XDG_STATE_HOME/survived" ]; then exec sleep 60; fi
: > "$XDG_STATE_HOME/survived"
kill -SEGV $$
"#;

const CRASHES_ALWAYS: &str = r#"#!/bin/sh
echo "fake shell is going down" >&2
kill -SEGV $$
"#;

#[test]
fn crashed_shell_restarts_and_leaves_a_report() {
    let harness = Harness::supervising(EXTENT, CRASHES_ONCE);

    let reports = eventually("a crash report", || {
        Some(crash_reports(harness.state())).filter(|reports| !reports.is_empty())
    });
    let report = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(report.starts_with("rikka-shell crashed"), "{report}");
    assert!(report.contains("SIGSEGV"), "{report}");
    assert!(report.contains("restart: in 500ms"), "{report}");
    assert!(report.contains(LAST_WORDS), "{report}");

    eventually("the restarted shell", || status(&harness).pid);
    let log = shell_log(harness.state());
    eventually("both runs in the shell log", || {
        let text = std::fs::read_to_string(&log).ok()?;
        (text.matches(LAST_WORDS).count() == 2).then_some(())
    });
    assert_eq!(crash_reports(harness.state()).len(), 1);
}

#[test]
fn shell_crashing_repeatedly_is_given_up() {
    let harness = Harness::supervising(EXTENT, CRASHES_ALWAYS);

    let reports = eventually("five crash reports", || {
        Some(crash_reports(harness.state())).filter(|reports| reports.len() == 5)
    });
    assert_eq!(status(&harness).pid, None);
    let given_up = reports
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path)
                .unwrap()
                .contains("restart: given up")
        })
        .count();
    assert_eq!(given_up, 1);
}

fn status(harness: &Harness) -> Status {
    harness.ask(Request::new(CAPABILITY, STATUS))
}

fn crash_reports(state: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(state.join("rikka/crashes")) else {
        return Vec::new();
    };
    let mut reports: Vec<PathBuf> = entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "log")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("rikka-shell-"))
        })
        .collect();
    reports.sort();
    reports
}

fn shell_log(state: &Path) -> PathBuf {
    state.join("rikka/compositors/headless-wayland-harness-shell.log")
}

fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(RECHECK);
    }
}
