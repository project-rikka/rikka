use std::ops::Range;
use std::time::{Duration, SystemTime};

use rikka_omnibox::{
    Action, Candidate, Candidates, MatchKind, Ranked, Request, SearchEngines, SearchSuggestion,
    Surface, classify,
};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Tab(u32),
    Archived(u32),
    Command(&'static str),
}

fn span(start: usize, end: usize) -> Range<usize> {
    start..end
}

const DAY: Duration = Duration::from_secs(86_400);

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn ago(days: u64) -> SystemTime {
    now() - DAY * days as u32
}

fn library() -> Candidates<Key> {
    let mut candidates = Candidates::new();
    let pushes = [
        Candidate::history("https://github.com/", "GitHub")
            .visits(40)
            .typed(6)
            .last_visit(ago(1)),
        Candidate::history(
            "https://github.com/rust-lang/rust",
            "rust-lang/rust: Empowering everyone",
        )
        .visits(12)
        .typed(1)
        .last_visit(ago(2)),
        Candidate::history("https://gitlab.com/explore", "Explore · GitLab")
            .visits(2)
            .last_visit(ago(40)),
        Candidate::history(
            "https://www.rust-lang.org/learn",
            "Learn Rust - Rust Programming Language",
        )
        .visits(9)
        .last_visit(ago(3)),
        Candidate::history(
            "https://doc.rust-lang.org/std/vec/struct.Vec.html",
            "Vec in std::vec - Rust",
        )
        .visits(30)
        .last_visit(ago(1)),
        Candidate::history(
            "https://weather.example.jp/tokyo",
            "東京の天気予報 - お天気ナビ",
        )
        .visits(5)
        .last_visit(ago(1)),
        Candidate::history(
            "https://ja.wikipedia.org/wiki/%E6%9D%B1%E4%BA%AC%E9%83%BD",
            "東京都 - Wikipedia",
        )
        .visits(3)
        .last_visit(ago(4)),
        Candidate::history("https://news.example.com/article/1", "Breaking news").visits(1),
        Candidate::open_tab(
            Key::Tab(1),
            "https://news.example.com/article/1",
            "Breaking news",
        ),
        Candidate::history("https://mail.example.com/inbox", "Inbox")
            .visits(80)
            .typed(3),
        Candidate::favorite(Key::Tab(2), "https://mail.example.com/inbox", "Inbox (3)"),
        Candidate::pinned(Key::Tab(3), "https://calendar.example.com/", "Calendar"),
        Candidate::archived(
            Key::Archived(9),
            "https://recipes.example.org/curry",
            "Curry recipe",
        )
        .last_visit(ago(20)),
        Candidate::history("http://intranet/wiki/Home", "Intranet wiki")
            .visits(4)
            .last_visit(ago(1)),
        Candidate::command(Key::Command("new-space"), "New Space").keywords(["create", "space"]),
        Candidate::command(Key::Command("settings"), "Settings")
            .keywords(["preferences", "options"]),
        Candidate::command(Key::Command("split"), "Toggle Split View"),
    ];
    for candidate in pushes {
        candidates.push(candidate).unwrap();
    }
    candidates
}

fn rank(
    candidates: &Candidates<Key>,
    text: &str,
    configure: impl FnOnce(Request<'_>) -> Request<'_>,
) -> Ranked<Key> {
    let engines = SearchEngines::builtin();
    let classified = classify(text, &engines);
    let request = configure(Request::new(&classified, &engines).now(now()));
    candidates.rank(&request)
}

fn navigates_to(ranked: &Ranked<Key>) -> Option<&str> {
    match &ranked.default_match()?.action {
        Action::Navigate(url) => Some(url.as_str()),
        _ => None,
    }
}

#[test]
fn inline_autocompletes_strong_hosts() {
    let candidates = library();
    let ranked = rank(&candidates, "git", |request| request);
    assert_eq!(ranked.inline.as_deref(), Some("hub.com"));
    assert_eq!(navigates_to(&ranked), Some("https://github.com/"));
    let first = ranked.default_match().unwrap();
    assert_eq!(first.title, "GitHub");
    assert_eq!(first.url, "github.com");
    assert_eq!(first.url_highlights, [span(0, 3)]);
    assert!(
        ranked
            .matches
            .iter()
            .all(|row| row.url != "gitlab.com/explore" || row.kind == MatchKind::History)
    );

    let path = rank(&candidates, "github.com/rust", |request| request);
    assert_eq!(path.inline.as_deref(), Some("-lang/rust"));
    assert_eq!(
        navigates_to(&path),
        Some("https://github.com/rust-lang/rust")
    );

    let upper = rank(&candidates, "GitHub.c", |request| request);
    assert_eq!(upper.inline.as_deref(), Some("om"));

    let scheme = rank(&candidates, "https://git", |request| request);
    assert_eq!(scheme.inline.as_deref(), Some("hub.com"));

    let www = rank(&candidates, "www.rust", |request| request);
    assert_eq!(www.inline.as_deref(), Some("-lang.org"));

    let exact = rank(&candidates, "github.com", |request| request);
    assert_eq!(exact.inline, None);
    assert_eq!(exact.default_match().unwrap().kind, MatchKind::TypedUrl);
    assert_eq!(exact.default_match().unwrap().title, "GitHub");
    assert_eq!(navigates_to(&exact), Some("https://github.com/"));
}

#[test]
fn no_inline_after_delete_or_outside_address() {
    let candidates = library();
    let deleted = rank(&candidates, "git", |request| request.just_deleted(true));
    assert_eq!(deleted.inline, None);
    assert_eq!(
        deleted.default_match().unwrap().kind,
        MatchKind::TypedSearch
    );
    assert!(deleted.matches.iter().any(|row| row.url == "github.com"));

    let spaced = rank(&candidates, "git ", |request| request);
    assert_eq!(spaced.inline, None);

    let bar = rank(&candidates, "git", |request| {
        request.surface(Surface::CommandBar)
    });
    assert_eq!(bar.inline, None);

    let weak = rank(&candidates, "gitl", |request| request);
    assert_eq!(weak.inline, None);
    assert_eq!(weak.default_match().unwrap().kind, MatchKind::TypedSearch);

    let keyword = rank(&candidates, "@ddg git", |request| request);
    assert_eq!(keyword.inline, None);
}

#[test]
fn dedups_urls_keeping_best_kind() {
    let candidates = library();
    let news = rank(&candidates, "breaking", |request| {
        request.surface(Surface::CommandBar)
    });
    let rows: Vec<_> = news
        .matches
        .iter()
        .filter(|row| row.url == "news.example.com/article/1")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, MatchKind::OpenTab);
    assert_eq!(rows[0].action, Action::SwitchToTab(Key::Tab(1)));

    let inbox = rank(&candidates, "inbox", |request| {
        request.surface(Surface::CommandBar)
    });
    let rows: Vec<_> = inbox
        .matches
        .iter()
        .filter(|row| row.url == "mail.example.com/inbox")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, MatchKind::Favorite);
    assert_eq!(rows[0].action, Action::SwitchToTab(Key::Tab(2)));
    assert!(rows[0].relevance > 1000);

    let curry = rank(&candidates, "curry", |request| {
        request.surface(Surface::CommandBar)
    });
    let archived = curry
        .matches
        .iter()
        .find(|row| row.kind == MatchKind::Archived)
        .unwrap();
    assert_eq!(archived.action, Action::RestoreArchived(Key::Archived(9)));
}

#[test]
fn matches_japanese_substrings() {
    let candidates = library();
    let weather = rank(&candidates, "天気", |request| request);
    assert_eq!(
        weather.default_match().unwrap().kind,
        MatchKind::TypedSearch
    );
    let row = weather
        .matches
        .iter()
        .find(|row| row.kind == MatchKind::History)
        .unwrap();
    assert_eq!(row.title, "東京の天気予報 - お天気ナビ");
    assert_eq!(row.title_highlights, [span(9, 15)]);

    let tokyo = rank(&candidates, "東京都", |request| request);
    let wiki = tokyo
        .matches
        .iter()
        .find(|row| row.url.starts_with("ja.wikipedia.org"))
        .unwrap();
    assert_eq!(wiki.url, "ja.wikipedia.org/wiki/東京都");
    assert_eq!(wiki.title_highlights, [span(0, 9)]);
    assert_eq!(wiki.url_highlights, [span(22, 31)]);

    let mixed = rank(&candidates, "東京 wiki", |request| request);
    assert!(
        mixed
            .matches
            .iter()
            .any(|row| row.url.starts_with("ja.wikipedia.org"))
    );
    assert!(
        !mixed
            .matches
            .iter()
            .any(|row| row.url.starts_with("weather"))
    );
}

#[test]
fn merges_search_suggestions() {
    let candidates = library();
    let suggestions = [
        SearchSuggestion::query("rust"),
        SearchSuggestion::query("rust language"),
        SearchSuggestion::query("Rust Language"),
        SearchSuggestion::navigation(Url::parse("https://www.rust-lang.org/learn").unwrap()),
        SearchSuggestion::navigation(Url::parse("https://crates.io/").unwrap()),
        SearchSuggestion::query("rust game"),
    ];
    let engines = SearchEngines::builtin();
    let classified = classify("@ddg rust", &engines);
    let request = Request::new(&classified, &engines)
        .now(now())
        .suggestions(&suggestions);
    let ranked = candidates.rank(&request);
    let first = ranked.default_match().unwrap();
    assert_eq!(first.kind, MatchKind::TypedSearch);
    let texts: Vec<&str> = ranked
        .matches
        .iter()
        .filter(|row| row.kind == MatchKind::Suggestion)
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(texts, ["rust language", "rust game"]);
    for row in ranked
        .matches
        .iter()
        .filter(|row| row.kind == MatchKind::Suggestion)
    {
        let Action::Search(search) = &row.action else {
            panic!("{row:?}");
        };
        assert_eq!(search.engine(), "duckduckgo");
        assert!(search.by_keyword());
        assert_eq!(row.title_highlights, [span(0, 4)]);
    }
    let urls: Vec<&str> = ranked
        .matches
        .iter()
        .filter(|row| row.kind == MatchKind::SuggestedUrl)
        .map(|row| row.url.as_str())
        .collect();
    assert_eq!(urls, ["crates.io"]);
}

#[test]
fn rejects_invalid_candidate_urls() {
    let mut candidates = Candidates::<u64>::new();
    assert!(
        candidates
            .push(Candidate::history("not a url", "x"))
            .is_err()
    );
    assert!(
        candidates
            .push(Candidate::open_tab(1, "https://example.com/", "x"))
            .is_ok()
    );
    assert_eq!(candidates.len(), 1);
}
