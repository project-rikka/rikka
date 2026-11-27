mod support;

use rikka_web::{
    CloseReason, ContextLink, EditCommand, Event, Run, RuntimeEvent, Script, Session,
    SessionSettings,
};
use serde_json::{Value, json};

use crate::support::{Engine, block_on, next_matching, served, until_page};

const PROBE: Script = Script::bundled("rikka-web-test/probe.js");

const PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Rikka</title><p>hello</p>";

const OTHER_PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Other</title><p>elsewhere</p>";

const MENU_PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Menu</title><p id=text>hello world</p><a id=link href=\"/next\">next page</a><div id=own>own</div><input id=field><script>document.getElementById(\"own\").addEventListener(\"contextmenu\", (event) => event.preventDefault())</script>";

fn probe(session: &Session, function: &str, id: &str) -> Value {
    let results =
        block_on(session.run(Run::load(PROBE).call(function).arg(id))).expect("the probe runs");
    results[0].value.clone()
}

#[test]
fn session_loads_a_page_and_runs_bundled_scripts() {
    let engine = Engine::start();
    let url = served(PAGE);
    let session = engine
        .session(SessionSettings::new().url(&url))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Rikka");
    assert_eq!(session.page().url, url);

    let results = block_on(session.run(Run::load(PROBE).call("probe.title")))
        .expect("the bundled script runs");
    assert_eq!(results[0].value, json!("Rikka"));

    block_on(
        session.run(
            Run::load(PROBE)
                .call("probe.post")
                .arg("greeting")
                .arg(json!({ "n": 1 })),
        ),
    )
    .expect("the probe posts");
    let message = next_matching(&mut events, |event| match event {
        Event::Message(message) => Some(message),
        _ => None,
    });
    assert_eq!(
        (message.name.as_str(), &message.body),
        ("greeting", &json!({ "n": 1 }))
    );
}

#[test]
fn content_scripts_ask_and_load_bundled_scripts() {
    let engine = Engine::start();
    let session = engine
        .session(SessionSettings::new().url(served(PAGE)))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Rikka");

    let asking = session.run(
        Run::load(PROBE)
            .call("probe.ask")
            .arg("question")
            .arg(json!({ "n": 2 })),
    );
    let ask = next_matching(&mut events, |event| match event {
        Event::Ask(ask) => Some(ask),
        _ => None,
    });
    assert_eq!(
        (ask.name.as_str(), &ask.body),
        ("question", &json!({ "n": 2 }))
    );
    ask.respond(json!("answer"));
    let answered = block_on(asking).expect("the ask settles");
    assert_eq!(answered[0].value, json!("answer"));

    for (script, expected) in [
        (
            "rikka-web-test/loaded.js",
            json!({ "loaded": true, "value": "loaded" }),
        ),
        (
            "../manifest.json",
            json!({ "loaded": false, "value": "loaded" }),
        ),
    ] {
        let results = block_on(session.run(Run::load(PROBE).call("probe.load").arg(script)))
            .expect("the probe loads");
        assert_eq!(results[0].value, expected, "{script}");
    }
}

#[test]
fn closing_a_session_ends_it() {
    let engine = Engine::start();
    let session = engine
        .session(SessionSettings::new().url(served(PAGE)))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Rikka");
    session.close();
    let reason = next_matching(&mut events, |event| match event {
        Event::Closed(reason) => Some(reason),
        _ => None,
    });
    assert_eq!(reason, CloseReason::Requested);
    assert!(!session.is_open());
}

#[test]
fn back_and_forward_follow_cross_origin_navigation() {
    let engine = Engine::start();
    let first = served(PAGE);
    let second = served(OTHER_PAGE);
    let session = engine
        .session(SessionSettings::new().url(&first))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Rikka");
    assert!(!session.page().can_go_back);

    session.load(&second);
    let started = next_matching(&mut events, |event| match event {
        Event::Started { url } => Some(url),
        _ => None,
    });
    assert_eq!(started, second);
    until_page(&session, &mut events, |page| {
        page.title == "Other" && page.can_go_back && !page.can_go_forward
    });

    session.back();
    until_page(&session, &mut events, |page| {
        page.title == "Rikka" && !page.can_go_back && page.can_go_forward
    });

    session.forward();
    until_page(&session, &mut events, |page| {
        page.title == "Other" && page.can_go_back && !page.can_go_forward
    });
}

#[test]
fn history_records_visited_pages() {
    let mut engine = Engine::start();
    let url = served(PAGE);
    let session = engine
        .session(SessionSettings::new().url(&url))
        .expect("a session opens");
    let visited = next_matching(&mut engine.events, |event| match event {
        RuntimeEvent::Visited(item) => Some(item),
        _ => None,
    });
    assert_eq!(visited.url, url);
    let found = block_on(engine.history().search(&rikka_web::HistoryQuery::new("")))
        .expect("history answers");
    assert!(found.iter().any(|item| item.url == url));
    drop(session);
}

#[test]
fn context_menus_report_only_what_the_page_leaves_to_us() {
    let engine = Engine::start();
    let url = served(MENU_PAGE);
    let session = engine
        .session(SessionSettings::new().url(&url))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Menu");

    probe(&session, "probe.select", "text");
    probe(&session, "probe.contextmenu", "own");
    probe(&session, "probe.contextmenu", "link");
    let menu = next_matching(&mut events, |event| match event {
        Event::ContextMenu(menu) => Some(menu),
        _ => None,
    });
    assert_eq!(
        menu.link,
        Some(ContextLink {
            url: format!("{url}next"),
            text: "next page".into(),
        })
    );
    assert_eq!(
        (menu.frame, menu.at, menu.selection.as_str(), menu.editable),
        (0, None, "hello world", None)
    );
    assert_eq!(
        block_on(session.selection(0)).expect("the selection reads"),
        "hello world"
    );

    probe(&session, "probe.focus", "field");
    session.edit(0, EditCommand::Paste("typed".into()));
    assert_eq!(probe(&session, "probe.value", "field"), json!("typed"));
}

#[test]
fn hovered_links_show_until_the_pointer_leaves_them() {
    let engine = Engine::start();
    let url = served(MENU_PAGE);
    let session = engine
        .session(SessionSettings::new().url(&url))
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    until_page(&session, &mut events, |page| page.title == "Menu");

    let next = format!("{url}next");
    probe(&session, "probe.hover", "link");
    until_page(&session, &mut events, |page| {
        page.hovered_link.as_deref() == Some(next.as_str())
    });
    probe(&session, "probe.hover", "text");
    until_page(&session, &mut events, |page| page.hovered_link.is_none());
}
