use std::cell::{Cell, RefCell};
use std::future::Future;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::time::Duration;

use browser_translate::{CodeMode, Command, Display, MESSAGE, Message, Watch, probe_script};
use futures_util::{Stream, StreamExt};
use rikka_languages::Language;
use rikka_types::Choice;
use rikka_web::{
    Event, Frames, Run, Runtime, RuntimeEvent, Script, ScriptAsk, Session, SessionSettings,
};
use serde_json::{Value, json};
use tokio::sync::Notify;

const PATIENCE: Duration = Duration::from_secs(30);
const STARTUP: Duration = Duration::from_secs(45);
const QUIET: Duration = Duration::from_millis(800);
const README: &str = "/BurntSushi/ripgrep";
const BLOB: &str = "/BurntSushi/ripgrep/blob/master/crates/core/main.rs";
const DOCS: &str = "/docs/start/";
const LATE: &str = "/late";
const FRAMED: &str = "/framed";
const STRICT: &str = "/strict";
const STRICT_POLICY: &str = "Content-Security-Policy: default-src 'none'; frame-src 'self'\r\n";
const FIXTURE: Script = Script::bundled("browser-translate-test/page.js");
const REACHED: &str = "browser-translate-test.reached";

fn page_at(path: &str) -> Option<String> {
    Some(match path {
        README => include_str!("fixtures/readme.html").to_owned(),
        BLOB => include_str!("fixtures/blob.html").to_owned(),
        DOCS => include_str!("fixtures/docs.html").to_owned(),
        STRICT => "<!DOCTYPE html><html lang=\"en\"><head><title>Strict</title></head><body><p id=\"strict\">This page forbids every script of its own.</p></body></html>".to_owned(),
        FRAMED => "<!DOCTYPE html><html lang=\"en\"><head><title>Framed</title></head><body><p id=\"outer\">Text outside the frame.</p><iframe id=\"frame\" src=\"/frame\"></iframe></body></html>".to_owned(),
        "/frame" => "<!DOCTYPE html><html lang=\"en\"><body><p id=\"inner\">Text inside the frame.</p></body></html>".to_owned(),
        LATE => "<!DOCTYPE html><html lang=\"en\"><head><title>Late</title></head><body><main>Loading</main><script>addEventListener('load', () => setTimeout(() => { document.querySelector('main').innerHTML = '<p>The article arrives after the page has loaded, the way single page applications render their content once their data comes back from the network.</p><p>A second paragraph makes the sample long enough to count as the page.</p>'; }, 1500));</script></body></html>".to_owned(),
        "/elsewhere" => "<!DOCTYPE html><html lang=\"en\"><head><title>Elsewhere</title></head><body><p>Another page entirely.</p></body></html>".to_owned(),
        _ => return None,
    })
}

fn site() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let base = format!("http://{}", listener.local_addr().expect("local address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                return;
            };
            let mut line = String::new();
            let Ok(reader) = stream.try_clone() else {
                continue;
            };
            let mut reader = BufReader::new(reader);
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let policy = if path == STRICT { STRICT_POLICY } else { "" };
            let reply = match page_at(path) {
                Some(body) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n{policy}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                ),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_owned(),
            };
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    base
}

fn html(select: &str) -> Value {
    json!({ "select": select, "read": "html" })
}

fn outer(select: &str) -> Value {
    json!({ "select": select, "read": "outer" })
}

fn text(select: &str) -> Value {
    json!({ "select": select, "read": "text" })
}

fn value(select: &str) -> Value {
    json!({ "select": select, "read": "value" })
}

fn attribute(select: &str, name: &str) -> Value {
    json!({ "select": select, "read": "attribute", "attribute": name })
}

fn body() -> Value {
    json!({ "read": "html" })
}

fn title() -> Value {
    json!({ "read": "title" })
}

fn framed(frame: &str, mut query: Value) -> Value {
    query["frame"] = Value::from(frame);
    query
}

fn first_file() -> Value {
    json!({ "select": ".react-code-file-contents:not(#virtual) .react-file-line", "all": true, "read": "text" })
}

fn second_file() -> Value {
    json!({ "select": "#virtual .react-file-line", "all": true, "read": "code" })
}

fn translated_level(nodes: &[Value]) -> Vec<Value> {
    nodes
        .iter()
        .rev()
        .map(|node| match node {
            Value::String(text) => Value::String(text.to_uppercase()),
            Value::Object(object) => match object.get("nodes").and_then(Value::as_array) {
                Some(children) => json!({ "i": object["i"], "nodes": translated_level(children) }),
                None => json!({ "i": object["i"] }),
            },
            other => panic!("expected string or object node, got {other}"),
        })
        .collect()
}

fn ids(nodes: &[Value], seen: &mut Vec<u64>, chars: &mut usize) {
    for node in nodes {
        match node {
            Value::String(text) => *chars += text.chars().count(),
            Value::Object(object) => {
                seen.push(object["i"].as_u64().expect("element id"));
                match object.get("nodes").and_then(Value::as_array) {
                    Some(children) => ids(children, seen, chars),
                    None => {
                        *chars += object["keep"]
                            .as_str()
                            .expect("kept slot text")
                            .chars()
                            .count()
                    }
                }
            }
            other => panic!("expected string or object node, got {other}"),
        }
    }
}

fn checked(units: &[Value]) {
    assert!(units.len() <= 40, "{} units in one message", units.len());
    let mut total = 0;
    for unit in units {
        assert!(unit["pre"].is_boolean(), "{unit}");
        let mut seen = Vec::new();
        let mut chars = 0;
        ids(
            unit["nodes"].as_array().expect("unit nodes"),
            &mut seen,
            &mut chars,
        );
        let expected: Vec<u64> = (0..seen.len() as u64).collect();
        assert_eq!(seen, expected, "ids run 0.. in document order: {unit}");
        total += chars;
    }
    assert!(
        units.len() == 1 || total <= 6000,
        "{total} characters in one message"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Committed,
    Finished,
}

#[derive(Default)]
struct Host {
    command: Option<Command>,
    pending: Vec<(Watch, ScriptAsk)>,
    watches: Vec<Watch>,
    pages: Vec<Value>,
    page_reported: Rc<Notify>,
    reached: Vec<u64>,
    reached_notify: Rc<Notify>,
    navigations: Vec<(Step, String)>,
    navigated: Rc<Notify>,
    units: Vec<Value>,
    batches: usize,
    batched: Rc<Notify>,
}

impl Host {
    fn answer(&self, watch: &Watch) -> Option<Value> {
        let command = match &self.command {
            None => watch.session.is_some().then_some(Command::Restore)?,
            Some(
                command @ Command::Translate {
                    session, display, ..
                },
            ) => {
                if watch.session == Some(*session) && watch.display == Some(*display) {
                    return None;
                }
                command.clone()
            }
            Some(Command::Restore) => unreachable!("the host only wants translations"),
        };
        Some(command.answer())
    }

    fn watch(&mut self, watch: Watch, ask: ScriptAsk) {
        match self.answer(&watch) {
            Some(answer) => ask.respond(answer),
            None => self.pending.push((watch, ask)),
        }
    }

    fn want(&mut self, command: Option<Command>) {
        self.command = command;
        for (watch, ask) in std::mem::take(&mut self.pending) {
            self.watch(watch, ask);
        }
    }

    fn translate(&mut self, body: &Value, ask: ScriptAsk) {
        let units = body["units"].as_array().cloned().unwrap_or_default();
        checked(&units);
        self.batches += 1;
        self.batched.notify_waiters();
        let current = match &self.command {
            Some(Command::Translate { session, .. }) => Some(*session),
            _ => None,
        };
        if body["session"].as_u64() != current {
            drop(ask);
            return;
        }
        let outputs: Vec<Value> = units
            .iter()
            .map(|unit| json!({ "nodes": translated_level(unit["nodes"].as_array().expect("unit nodes")) }))
            .collect();
        self.units.extend(units);
        ask.respond(json!({ "units": outputs }));
    }

    fn asked(&mut self, ask: ScriptAsk) {
        match Message::parse(&ask.body) {
            Ok(Message::Watch(watch)) => {
                self.watches.push(watch);
                self.watch(watch, ask);
            }
            Ok(Message::Translate(_)) => {
                let body = ask.body.clone();
                self.translate(&body, ask);
            }
            other => panic!("unexpected ask {}: {other:?}", ask.body),
        }
    }

    fn navigation(&mut self, step: Step, url: String) {
        self.navigations.push((step, url));
        self.navigated.notify_waiters();
    }

    fn posted(&mut self, body: Value) {
        match Message::parse(&body) {
            Ok(Message::Page(_)) => {
                self.pages.push(body);
                self.page_reported.notify_waiters();
            }
            other => panic!("unexpected post {body}: {other:?}"),
        }
    }
}

async fn serve(mut events: impl Stream<Item = Event> + Unpin, host: Rc<RefCell<Host>>) {
    while let Some(event) = events.next().await {
        match event {
            Event::Committed { url, .. } => host.borrow_mut().navigation(Step::Committed, url),
            Event::Finished { url } => host.borrow_mut().navigation(Step::Finished, url),
            Event::Closed(reason) => panic!("session closed: {reason:?}"),
            Event::Message(message) if message.name == MESSAGE => {
                host.borrow_mut().posted(message.body);
            }
            Event::Message(message) if message.name == REACHED => {
                let token = message.body.as_u64().expect("condition token");
                let mut host = host.borrow_mut();
                host.reached.push(token);
                host.reached_notify.notify_waiters();
            }
            Event::Ask(ask) if ask.name == MESSAGE => host.borrow_mut().asked(ask),
            _ => {}
        }
    }
}

async fn patiently<T>(what: &str, wait: impl Future<Output = T>) -> T {
    tokio::time::timeout(PATIENCE, wait)
        .await
        .unwrap_or_else(|_| panic!("{what} did not happen in time"))
}

struct Page {
    view: Session,
    site: String,
    host: Rc<RefCell<Host>>,
    conditions: Cell<u64>,
}

impl Page {
    async fn run(&self, run: Run) -> Vec<Value> {
        let described = format!("{run:?}");
        patiently(&described, self.view.run(run))
            .await
            .unwrap_or_else(|error| panic!("{described} failed: {error}"))
            .into_iter()
            .map(|result| result.value)
            .collect()
    }

    async fn call(&self, function: &str, args: impl IntoIterator<Item = Value>) -> Value {
        let run = args
            .into_iter()
            .fold(Run::load(FIXTURE).call(function), Run::arg);
        self.run(run)
            .await
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{function} gave no frame result"))
    }

    async fn read(&self, query: Value) -> Value {
        self.call("test.read", [query]).await
    }

    async fn string(&self, query: Value) -> String {
        match self.read(query.clone()).await {
            Value::String(text) => text,
            other => panic!("{query} gave {other}"),
        }
    }

    async fn until(&self, query: Value, expected: impl Into<Value>) {
        let expected = expected.into();
        let token = self.conditions.get() + 1;
        self.conditions.set(token);
        let reached = Rc::clone(&self.host.borrow().reached_notify);
        let next = reached.notified();
        self.call(
            "test.until",
            [Value::from(token), query.clone(), expected.clone()],
        )
        .await;
        let waited = tokio::time::timeout(PATIENCE, async {
            let mut next = std::pin::pin!(next);
            loop {
                if self.host.borrow().reached.contains(&token) {
                    return;
                }
                next.as_mut().await;
                next.set(reached.notified());
            }
        })
        .await;
        if waited.is_err() {
            let last = self.read(query.clone()).await;
            panic!("{query} never gave {expected}, last {last}");
        }
    }

    async fn until_reported(&self, what: &str, mut reached: impl FnMut(&[Value]) -> bool) {
        let reported = Rc::clone(&self.host.borrow().page_reported);
        patiently(what, async {
            loop {
                let next = reported.notified();
                if reached(&self.host.borrow().pages) {
                    return;
                }
                next.await;
            }
        })
        .await;
    }

    fn arrivals(&self) -> usize {
        self.host.borrow().navigations.len()
    }

    async fn arrived(&self, path: &str, step: Step, since: usize) {
        let navigated = Rc::clone(&self.host.borrow().navigated);
        let expected = (step, format!("{}{path}", self.site));
        let arrived = tokio::time::timeout(PATIENCE, async {
            loop {
                let next = navigated.notified();
                if self.host.borrow().navigations[since..].contains(&expected) {
                    return;
                }
                next.await;
            }
        })
        .await;
        if arrived.is_err() {
            let seen = self.host.borrow().navigations[since..].to_vec();
            panic!(
                "never reached {expected:?}, saw {seen:?}, page {:?}",
                self.view.page()
            );
        }
    }

    async fn loaded(&self, path: &str) {
        let since = self.arrivals();
        self.view.load(format!("{}{path}", self.site));
        self.arrived(path, Step::Finished, since).await;
    }

    async fn quiet(&self) {
        let batched = Rc::clone(&self.host.borrow().batched);
        patiently("translation requests settling", async {
            while tokio::time::timeout(QUIET, batched.notified())
                .await
                .is_ok()
            {}
        })
        .await;
    }

    fn translate(&self, session: u64, display: &str, code: &str) {
        self.host.borrow_mut().want(Some(Command::Translate {
            session,
            to: Language::lookup("ja").expect("Japanese"),
            display: Display::from_slug(display).expect("display slug"),
            code: CodeMode::from_slug(code).expect("code slug"),
        }));
    }

    fn restore(&self) {
        self.host.borrow_mut().want(None);
    }

    fn batches(&self) -> usize {
        self.host.borrow().batches
    }

    async fn translators(&self) -> usize {
        self.run(
            Run::load(FIXTURE)
                .call("test.translator")
                .frames(Frames::All),
        )
        .await
        .iter()
        .filter(|loaded| **loaded == Value::Bool(true))
        .count()
    }

    fn requested(&self) -> String {
        Value::Array(self.host.borrow().units.clone()).to_string()
    }
}

fn scenario<F, Fut>(play: F)
where
    F: FnOnce(Page) -> Fut,
    Fut: Future<Output = ()>,
{
    let site = site();
    let engine = Runtime::builder().start().expect("Chrome starts");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("create tokio runtime");
    let mut started = engine.events().expect("events not yet taken");
    runtime.block_on(async {
        tokio::time::timeout(STARTUP, async {
            while let Some(event) = started.next().await {
                match event {
                    RuntimeEvent::Ready { .. } => return,
                    RuntimeEvent::Failed(failure) => panic!("the extension failed: {failure:?}"),
                    RuntimeEvent::Exited(exit) => panic!("Chrome exited: {exit:?}"),
                    _ => {}
                }
            }
        })
        .await
        .expect("Chrome started in time");
    });
    engine.register(&probe_script());
    let view = engine
        .session(SessionSettings::new())
        .expect("open session");
    let events = view.events().expect("events not yet taken");
    runtime.block_on(async {
        let host = Rc::new(RefCell::new(Host::default()));
        let page = Page {
            view: view.clone(),
            site,
            host: host.clone(),
            conditions: Cell::new(0),
        };
        tokio::select! {
            () = serve(events, host) => panic!("page events ended"),
            () = play(page) => {}
        }
    });
    drop(view);
    drop(started);
    drop(engine);
    rikka_web::wait_for_closed_runtimes();
}

#[test]
fn readme_translates_and_restores() {
    scenario(|page| async move {
        page.loaded(README).await;
        let original = page.string(body()).await;
        let title_text = page.string(title()).await;
        page.until_reported("the top frame page report", |pages| !pages.is_empty())
            .await;
        {
            let host = page.host.borrow();
            let report = host.pages.first().expect("top frame page report");
            assert_eq!(report["lang"], "en");
            let sample = report["sample"].as_str().expect("sample text");
            assert!(sample.contains("line-oriented search tool"), "{sample}");
            assert!(!sample.contains("cargo install"), "{sample}");
            assert!(!sample.contains("legacy_search"), "{sample}");
            assert!(
                host.watches
                    .iter()
                    .any(|watch| watch.top && watch.session.is_none())
            );
        }
        assert_eq!(
            page.translators().await,
            0,
            "no translator before a translation"
        );

        page.translate(7, "replace", "comments");
        page.until(
            html("#order"),
            "<em>FRIENDLY</em> AND <strong>FAST</strong>",
        )
        .await;
        page.until(title(), title_text.to_uppercase()).await;
        page.quiet().await;
        assert_eq!(
            page.translators().await,
            1,
            "the translator went to the one frame"
        );

        assert_eq!(
            page.string(outer("#intro code")).await,
            "<code>rg -uuu</code>"
        );
        let intro = page.string(text("#intro")).await;
        assert!(
            intro.contains("RIPGREP IS A LINE-ORIENTED SEARCH TOOL"),
            "{intro}"
        );
        assert!(intro.contains("rg -uuu"), "{intro}");
        assert_eq!(
            page.string(html("#icons")).await,
            " TO FIND A FILE QUICKLY.<span class=\"material-icons\">search</span> AND <kbd>Ctrl</kbd>PRESS "
        );
        assert_eq!(page.string(text("#heading")).await, "RIPGREP (RG)");
        assert_eq!(page.string(text("#tool")).await, "RIPGREP (UNICODE)");
        assert_eq!(
            page.string(html("#command")).await,
            "<code>rg -w '[A-Z]+_SUSPEND'</code>"
        );
        assert_eq!(page.string(text("#card div")).await, "USER GUIDE");
        assert_eq!(page.string(text("nav li a")).await, "CODE");
        assert_eq!(
            page.string(html("#ja")).await,
            "このツールはとても速いです。"
        );
        assert_eq!(
            page.string(html("#held")).await,
            "Do not translate this sentence."
        );

        assert_eq!(
            page.string(text("#plain pre")).await,
            "# INSTALL WITH CARGO FROM CRATES.IO\n$ cargo install ripgrep\n"
        );
        assert_eq!(
            page.string(text("#shell pre")).await,
            "bat README.md  # SHOW A FILE WITH SYNTAX HIGHLIGHTING\n\
             bat -n src/main.rs  # ONLY SHOW LINE NUMBERS\n\
             # INSTALL THE SHELL COMPLETIONS INTO YOUR\n\
             # PROFILE SO THAT THEY LOAD ON EVERY START.\n\
             rg --generate complete-bash > rg.bash"
        );
        assert_eq!(
            page.string(text("#rust pre")).await,
            "/// SEARCHES THE HAYSTACK FOR THE GIVEN PATTERN AND\n\
             /// REPORTS EVERY MATCH IT FINDS TO THE PRINTER.\n\
             fn search(pattern: &str) -> u64 {\n    \
             let count = 0; // STARTS AT ZERO\n    \
             // let old = legacy_search(pattern);\n    \
             /* THE PRINTER IS FLUSHED ONCE THE WHOLE\n       \
             FILE HAS BEEN SEARCHED. */\n    \
             #[cfg(unix)]\n    \
             count\n\
             }"
        );

        for (selector, name, expected) in [
            ("#attrs abbr", "title", "REGULAR EXPRESSION"),
            ("#attrs img", "alt", "A SCREENSHOT OF SEARCH RESULTS"),
            ("#attrs button", "aria-label", "COPY TO CLIPBOARD"),
            ("#search", "placeholder", "TYPE TO SEARCH"),
            ("#search", "aria-label", "SEARCH CODE"),
            ("#badges img", "alt", "BUILD STATUS"),
        ] {
            assert_eq!(
                page.string(attribute(selector, name)).await,
                expected,
                "{selector}[{name}]"
            );
        }

        let requested = page.requested();
        for code in [
            "cargo install",
            "legacy_search",
            "complete-bash",
            "#[cfg",
            "fn search",
            "このツール",
            "Do not translate",
        ] {
            assert!(!requested.contains(code), "{code:?} was sent: {requested}");
        }
        let only_kept = page.host.borrow().units.iter().any(|unit| {
            unit["nodes"]
                .as_array()
                .is_some_and(|nodes| nodes.iter().all(|node| node.get("keep").is_some()))
        });
        assert!(
            !only_kept,
            "a unit made only of kept slots was sent: {requested}"
        );

        page.restore();
        page.until(title(), title_text).await;
        page.until(body(), original).await;
    });
}

#[test]
fn code_view_translates_comments_only() {
    scenario(|page| async move {
        page.loaded(BLOB).await;
        let original = page.string(body()).await;
        let lines = page.string(text(".react-line-numbers")).await;
        let raw = page.string(value("textarea")).await;
        let virtual_lines = page.string(html("#virtual .react-code-lines")).await;
        let translated_second = json!([
            "// PRINT THE VERSION AND EXIT.",
            "fn version() {",
            "    println!(\"{}\", VERSION); // GOES TO STANDARD OUTPUT",
            "}",
        ]);

        page.translate(1, "replace", "comments");
        page.until(text("#LC2"), "THE MAIN ENTRY POINT INTO RIPGREP.")
            .await;
        page.until(second_file(), translated_second.clone()).await;
        page.quiet().await;

        assert_eq!(
            page.read(first_file()).await,
            json!([
                "/*!",
                "THE MAIN ENTRY POINT INTO RIPGREP.",
                "*/",
                "\n",
                "use std::{io::Write, process::ExitCode};",
                "\n",
                "// SINCE RUST NO LONGER USES JEMALLOC BY DEFAULT, RIPGREP WILL, BY DEFAULT,",
                "// USE THE SYSTEM ALLOCATOR. ON LINUX, THIS WOULD NORMALLY BE GLIBC'S",
                "// ALLOCATOR, WHICH IS PRETTY GOOD.",
                "//",
                "// HOWEVER, WHEN RIPGREP IS BUILT WITH MUSL, THIS MEANS RIPGREP WILL USE MUSL'S",
                "// ALLOCATOR, WHICH APPEARS TO BE SUBSTANTIALLY WORSE.",
                "#[cfg(all(target_env = \"musl\", target_pointer_width = \"64\"))]",
                "#[global_allocator]",
                "static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;",
                "\n",
                "/// THEN, AS IT WAS, THEN AGAIN IT WILL BE.",
                "fn main() -> ExitCode {",
                "    match run(flags::parse()) {",
                "        Ok(code) => code,",
                "        Err(err) => {",
                "            // LOOK FOR A BROKEN PIPE ERROR. IN THIS CASE, WE GENERALLY WANT",
                "            // TO EXIT \"GRACEFULLY\" WITH A SUCCESS EXIT CODE.",
                "            eprintln_locked!(\"{:#}\", err);",
                "            ExitCode::from(2)",
                "        }",
                "    }",
                "}",
            ])
        );
        assert_eq!(page.read(second_file()).await, translated_second);
        assert_eq!(page.string(text(".react-line-numbers")).await, lines);
        assert_eq!(page.string(value("textarea")).await, raw);
        assert!(!page.requested().contains("jemallocator::Jemalloc"));

        let batches = page.batches();
        page.call(
            "test.replace",
            [
                Value::from("#virtual .react-code-lines"),
                Value::from(virtual_lines),
            ],
        )
        .await;
        page.until(second_file(), translated_second).await;
        assert_eq!(
            page.batches(),
            batches,
            "a re-rendered file is translated from memory"
        );

        page.restore();
        page.until(body(), original).await;
    });
}

#[test]
fn late_content_is_resampled() {
    scenario(|page| async move {
        page.loaded(LATE).await;

        page.until_reported("late content", |pages| {
            pages.iter().any(|report| {
                report["sample"]
                    .as_str()
                    .is_some_and(|sample| sample.contains("The article arrives"))
            })
        })
        .await;
    });
}

#[test]
fn history_push_reports_new_page() {
    scenario(|page| async move {
        page.loaded(README).await;
        page.until_reported("the first page report", |pages| !pages.is_empty())
            .await;
        let before = page.host.borrow().pages.len();

        page.call("test.push", [Value::from("/spa-route")]).await;

        page.until_reported("history.pushState page", |pages| pages.len() > before)
            .await;
    });
}

#[test]
fn bfcache_restore_resumes_watch() {
    scenario(|page| async move {
        page.loaded(DOCS).await;
        page.translate(7, "replace", "comments");
        page.until(
            text("#closing"),
            "THAT IS ALL YOU NEED TO GET STARTED WITH THE LIBRARY.",
        )
        .await;
        page.call("test.remember", []).await;

        page.loaded("/elsewhere").await;
        page.until(text("p"), "ANOTHER PAGE ENTIRELY.").await;
        page.restore();
        page.until(text("p"), "Another page entirely.").await;
        let reports = page.host.borrow().pages.len();

        let since = page.arrivals();
        page.view.go(-1);
        page.arrived(DOCS, Step::Committed, since).await;
        assert_eq!(
            page.call("test.remembered", []).await,
            json!(true),
            "the page came back from the cache"
        );
        page.until(
            text("#closing"),
            "That is all you need to get started with the library.",
        )
        .await;
        page.until_reported("the restored page report", |pages| pages.len() > reports)
            .await;

        page.translate(8, "replace", "comments");
        page.until(
            text("#closing"),
            "THAT IS ALL YOU NEED TO GET STARTED WITH THE LIBRARY.",
        )
        .await;
    });
}

#[test]
fn frames_load_the_translator_when_translation_starts() {
    scenario(|page| async move {
        page.loaded(FRAMED).await;
        assert_eq!(page.translators().await, 0);

        page.translate(4, "replace", "comments");
        page.until(text("#outer"), "TEXT OUTSIDE THE FRAME.").await;
        page.until(framed("#frame", text("#inner")), "TEXT INSIDE THE FRAME.")
            .await;
        assert_eq!(
            page.translators().await,
            2,
            "the top frame and the inner frame"
        );

        page.restore();
        page.until(framed("#frame", text("#inner")), "Text inside the frame.")
            .await;
    });
}

#[test]
fn translation_runs_under_a_strict_content_security_policy() {
    scenario(|page| async move {
        page.loaded(STRICT).await;

        page.translate(2, "replace", "comments");
        page.until(
            text("#strict"),
            "THIS PAGE FORBIDS EVERY SCRIPT OF ITS OWN.",
        )
        .await;
    });
}
