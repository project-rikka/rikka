use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rikka_theme::Theme;
use rikka_ui::{Fetch, State, TypedUi, View, fetch, fetch_blocking, text};

use crate::support::wakes;

const FETCH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(State)]
struct Idle;

fn idle(_: &Idle) -> impl View {
    text("")
}

fn settled<T: Send + Sync + 'static>(mut ask: impl FnMut() -> Fetch<T>) -> Fetch<T> {
    let mut ui = TypedUi::new(Idle, idle, Theme::dark());
    let woken = wakes(&mut ui);
    loop {
        let answer = ask();
        if !answer.loading() {
            return answer;
        }
        assert!(woken.wait(FETCH_TIMEOUT), "fetch did not finish");
    }
}

#[test]
fn duplicate_fetch_runs_once() {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let ask = || {
        fetch::<usize, String, _>("duplicate fetch", || {
            RUNS.fetch_add(1, Ordering::Relaxed);
            async { Ok(7) }
        })
    };

    assert_eq!(settled(ask).ready().copied(), Some(7));
    assert!(matches!(ask(), Fetch::Ready(_)));
    assert_eq!(RUNS.load(Ordering::Relaxed), 1);

    let blocking = || fetch_blocking::<usize, String>("blocking work", || Ok(3));
    assert_eq!(settled(blocking).ready().copied(), Some(3));
}

#[test]
fn failed_fetch_reports_error_without_retry() {
    let ask =
        || fetch::<usize, String, _>("failing fetch", || async { Err("disk missing".to_owned()) });

    assert_eq!(settled(ask).failed(), Some("disk missing"));
}

#[test]
fn distinct_types_fetch_separately() {
    let text =
        settled(|| fetch::<String, String, _>("one name", || async { Ok("text".to_owned()) }));
    let count = settled(|| fetch::<usize, String, _>("one name", || async { Ok(4) }));

    assert_eq!(text.ready().map(String::as_str), Some("text"));
    assert_eq!(count.ready().copied(), Some(4));
}
