use browser_pages::{Arrival, Cause, VisitTracker, Visited};
use browser_store::Transition;

const PAGE: Arrival = Arrival {
    top_level: true,
    failed: false,
    redirect_source: false,
    redirect_target: false,
};
const REDIRECTING: Arrival = Arrival {
    redirect_source: true,
    ..PAGE
};
const REDIRECTED: Arrival = Arrival {
    redirect_target: true,
    ..PAGE
};
const HOP: Arrival = Arrival {
    redirect_source: true,
    redirect_target: true,
    ..PAGE
};

fn seen(tracker: &mut VisitTracker, url: &str, arrival: Arrival) -> Option<Transition> {
    tracker
        .visited(url, arrival)
        .map(|Visited { transition, .. }| transition)
}

#[test]
fn the_cause_set_by_the_app_names_the_next_visit_only() {
    let cases = [
        (Cause::Typed, Some(Transition::Typed)),
        (Cause::Generated, Some(Transition::Generated)),
        (Cause::Link, Some(Transition::Link)),
        (Cause::BackForward, Some(Transition::BackForward)),
        (Cause::Reload, Some(Transition::Reload)),
        (Cause::Restored, None),
    ];
    for (cause, expected) in cases {
        let mut tracker = VisitTracker::default();
        tracker.set_cause(cause);
        tracker.set_cause_if_unset(Cause::Generated);
        assert_eq!(
            seen(&mut tracker, "https://a.example/", PAGE),
            expected,
            "{cause:?}"
        );
        tracker.navigation_started();
        assert_eq!(
            seen(&mut tracker, "https://a.example/next", PAGE),
            Some(Transition::Link),
            "{cause:?} is spent"
        );
    }
}

#[test]
fn a_redirect_chain_keeps_the_cause_for_its_last_page() {
    let mut tracker = VisitTracker::default();
    tracker.set_cause(Cause::Typed);
    assert_eq!(
        seen(&mut tracker, "http://a.example/", REDIRECTING),
        Some(Transition::Redirect)
    );
    assert_eq!(
        seen(&mut tracker, "https://a.example/", HOP),
        Some(Transition::Redirect)
    );
    assert_eq!(
        seen(&mut tracker, "https://www.a.example/", REDIRECTED),
        Some(Transition::Typed)
    );
    tracker.navigation_started();
    assert_eq!(
        seen(&mut tracker, "https://www.a.example/about", PAGE),
        Some(Transition::Link)
    );
}

#[test]
fn a_restored_chain_records_nothing() {
    let mut tracker = VisitTracker::default();
    tracker.set_cause(Cause::Restored);
    assert_eq!(seen(&mut tracker, "http://a.example/", REDIRECTING), None);
    assert_eq!(seen(&mut tracker, "https://a.example/", REDIRECTED), None);
}

#[test]
fn page_decisions_tell_clicks_from_scripted_loads() {
    let mut tracker = VisitTracker::default();
    tracker.policy_decided("https://a.example/clicked", true);
    tracker.policy_decided("https://a.example/scripted", false);
    assert_eq!(
        seen(&mut tracker, "https://a.example/scripted", PAGE),
        Some(Transition::Redirect)
    );
    tracker.navigation_started();
    assert_eq!(
        seen(&mut tracker, "https://a.example/clicked", PAGE),
        Some(Transition::Link)
    );
}

#[test]
fn frames_failures_and_unrecorded_schemes_are_left_out() {
    let cases = [
        (
            "https://a.example/",
            Arrival {
                top_level: false,
                ..PAGE
            },
        ),
        (
            "https://a.example/",
            Arrival {
                failed: true,
                ..PAGE
            },
        ),
        ("about:blank", PAGE),
        ("moz-extension://id/page.html", PAGE),
    ];
    for (url, arrival) in cases {
        let mut tracker = VisitTracker::default();
        assert_eq!(seen(&mut tracker, url, arrival), None, "{url} {arrival:?}");
    }
    let mut tracker = VisitTracker::default();
    assert_eq!(
        seen(&mut tracker, "file:///home/a/page.html", PAGE),
        Some(Transition::Link)
    );
}
