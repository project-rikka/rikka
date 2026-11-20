use std::cell::RefCell;
use std::future::Future;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::time::Duration;

use browser_translate::{CodeMode, Command, Display, HANDLER, Message, PROBE_SCRIPT, WORLD, Watch};
use futures_util::{Stream, StreamExt};
use rikka_languages::Language;
use rikka_picture::SharedGpu;
use rikka_test_renderer::TestRenderer;
use rikka_types::Choice;
use rikka_web::{
    Event, Frames, InjectAt, MessageHandler, Runtime, RuntimeEvent, ScriptMessage, ScriptReply,
    Session, SessionSettings, UserContent, UserScript, World,
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
const REACHED: &str = "rikkaTestReached";

fn gpu() -> TestRenderer {
    let renderer =
        rikka_test_renderer::renderer("browser-translate-test").expect("Vulkan 1.3 device");
    SharedGpu::from_gpu(renderer.gpu()).publish();
    renderer
}

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

#[derive(Default)]
struct Host {
    command: Option<Command>,
    pending: Vec<(Watch, ScriptReply)>,
    watches: Vec<Watch>,
    translators: usize,
    pages: Vec<Value>,
    page_reported: Rc<Notify>,
    reached: Vec<u64>,
    reached_notify: Rc<Notify>,
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
        Some(command.answer(watch))
    }

    fn watch(&mut self, watch: Watch, reply: ScriptReply) {
        match self.answer(&watch) {
            Some(answer) => {
                if answer.get("translator").is_some() {
                    self.translators += 1;
                }
                reply.send(answer);
            }
            None => self.pending.push((watch, reply)),
        }
    }

    fn want(&mut self, command: Option<Command>) {
        self.command = command;
        for (watch, reply) in std::mem::take(&mut self.pending) {
            self.watch(watch, reply);
        }
    }

    fn translate(&mut self, body: &Value, reply: ScriptReply) {
        let units = body["units"].as_array().cloned().unwrap_or_default();
        checked(&units);
        self.batches += 1;
        self.batched.notify_waiters();
        let current = match &self.command {
            Some(Command::Translate { session, .. }) => Some(*session),
            _ => None,
        };
        if body["session"].as_u64() != current {
            reply.fail("stale session");
            return;
        }
        let outputs: Vec<Value> = units
            .iter()
            .map(|unit| json!({ "nodes": translated_level(unit["nodes"].as_array().expect("unit nodes")) }))
            .collect();
        self.units.extend(units);
        reply.send(json!({ "units": outputs }));
    }

    fn receive(&mut self, message: ScriptMessage) {
        let reply = message.reply.expect("replying handler");
        let body = message.body;
        match body["kind"].as_str() {
            Some("watch") => match Message::parse(&body) {
                Ok(Message::Watch(watch)) => {
                    self.watches.push(watch);
                    self.watch(watch, reply);
                }
                other => panic!("malformed watch {body}: {other:?}"),
            },
            Some("page") => {
                self.pages.push(body);
                self.page_reported.notify_waiters();
                reply.send(Value::Null);
            }
            Some("translate") => self.translate(&body, reply),
            _ => panic!("unknown message: {body}"),
        }
    }
}

async fn serve(mut events: impl Stream<Item = Event> + Unpin, host: Rc<RefCell<Host>>) {
    while let Some(event) = events.next().await {
        match event {
            Event::PageStarted(_) => host.borrow_mut().pending.clear(),
            Event::Changed | Event::PageStopped { .. } => host.borrow().navigated.notify_waiters(),
            Event::Closed(reason) => panic!("session closed: {reason:?}"),
            Event::LoadError(failure) => {
                panic!("{:?} did not load: {:?}", failure.uri, failure.error)
            }
            Event::ScriptMessage(message) if message.name == HANDLER => {
                host.borrow_mut().receive(message)
            }
            Event::ScriptMessage(message) if message.name == REACHED => {
                let token = message.body.as_u64().expect("condition token");
                let mut host = host.borrow_mut();
                host.reached.push(token);
                host.reached_notify.notify_waiters();
            }
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
    conditions: std::cell::Cell<u64>,
}

impl Page {
    async fn eval(&self, script: &str) -> Value {
        self.view
            .evaluate(script, &World::Page)
            .await
            .unwrap_or_else(|error| panic!("{script} failed: {error}"))
    }

    async fn text(&self, script: &str) -> String {
        match self.eval(script).await {
            Value::String(text) => text,
            other => panic!("{script} gave {other}"),
        }
    }

    async fn until(&self, script: &str, expected: impl Into<Value>) {
        self.until_in("document", script, expected).await;
    }

    async fn until_in(&self, root: &str, script: &str, expected: impl Into<Value>) {
        let expected = expected.into();
        let token = self.conditions.get() + 1;
        self.conditions.set(token);
        let reached = Rc::clone(&self.host.borrow().reached_notify);
        let next = reached.notified();
        self.eval(&format!(
            "(() => {{
                const expected = JSON.stringify({expected});
                const observer = new MutationObserver(() => check());
                const check = () => {{
                    let value;
                    try {{ value = ({script}); }} catch (error) {{ return false; }}
                    if (JSON.stringify(value) !== expected) return false;
                    observer.disconnect();
                    window.rikka.messageHandlers.{REACHED}.postMessage({token});
                    return true;
                }};
                if (!check()) observer.observe({root}, {{ subtree: true, childList: true, characterData: true, attributes: true }});
                return true;
            }})()"
        ))
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
            let last = self.eval(script).await;
            panic!("{script} never gave {expected}, last {last}");
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

    async fn at(&self, path: &str) {
        let navigated = Rc::clone(&self.host.borrow().navigated);
        let expected = format!("{}{path}", self.site);
        patiently(&format!("arriving at {path}"), async {
            loop {
                let next = navigated.notified();
                let page = self.view.page();
                if page.uri == expected && !page.loading {
                    return;
                }
                next.await;
            }
        })
        .await;
    }

    async fn loaded(&self, path: &str) {
        self.view.load(format!("{}{path}", self.site));
        self.at(path).await;
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

    fn translators(&self) -> usize {
        self.host.borrow().translators
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
    let _gpu = gpu();
    let site = site();
    let engine = Runtime::builder().start().expect("start installed engine");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("create tokio runtime");
    let mut started = engine.events().expect("events not yet taken");
    runtime.block_on(async {
        tokio::time::timeout(STARTUP, async {
            while let Some(event) = started.next().await {
                match event {
                    RuntimeEvent::Started { .. } => return,
                    RuntimeEvent::Exited(exit) => panic!("engine exited: {exit:?}"),
                    _ => {}
                }
            }
        })
        .await
        .expect("engine started in time");
    });
    let view = engine
        .session(SessionSettings::new())
        .expect("open session");
    view.set_user_content(
        UserContent::new()
            .script(
                UserScript::new("rikka-translate", PROBE_SCRIPT)
                    .world(World::isolated(WORLD))
                    .frames(Frames::All)
                    .inject_at(InjectAt::End),
            )
            .handler(
                MessageHandler::new(HANDLER)
                    .world(World::isolated(WORLD))
                    .with_replies(),
            )
            .handler(MessageHandler::new(REACHED)),
    );
    let events = view.events().expect("events not yet taken");
    runtime.block_on(async {
        let host = Rc::new(RefCell::new(Host::default()));
        let page = Page {
            view: view.clone(),
            site,
            host: host.clone(),
            conditions: std::cell::Cell::new(0),
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

const BODY: &str = "document.body.innerHTML";

#[test]
fn readme_translates_and_restores() {
    scenario(|page| async move {
        page.loaded(README).await;
        let original = page.text(BODY).await;
        let title = page.text("document.title").await;
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
                    .any(|watch| watch.top && watch.session.is_none() && !watch.translator)
            );
        }
        assert_eq!(page.translators(), 0, "no translator before a translation");

        page.translate(7, "replace", "comments");
        page.until(
            "document.querySelector('#order').innerHTML",
            "<em>FRIENDLY</em> AND <strong>FAST</strong>",
        )
        .await;
        page.until("document.title", title.to_uppercase()).await;
        page.quiet().await;
        assert_eq!(
            page.translators(),
            1,
            "the translator went to the one frame"
        );

        assert_eq!(
            page.text("document.querySelector('#intro code').outerHTML")
                .await,
            "<code>rg -uuu</code>"
        );
        let intro = page
            .text("document.querySelector('#intro').textContent")
            .await;
        assert!(
            intro.contains("RIPGREP IS A LINE-ORIENTED SEARCH TOOL"),
            "{intro}"
        );
        assert!(intro.contains("rg -uuu"), "{intro}");
        assert_eq!(
            page.text("document.querySelector('#icons').innerHTML")
                .await,
            " TO FIND A FILE QUICKLY.<span class=\"material-icons\">search</span> AND <kbd>Ctrl</kbd>PRESS "
        );
        assert_eq!(
            page.text("document.querySelector('#heading').textContent")
                .await,
            "RIPGREP (RG)"
        );
        assert_eq!(
            page.text("document.querySelector('#tool').textContent")
                .await,
            "RIPGREP (UNICODE)"
        );
        assert_eq!(
            page.text("document.querySelector('#command').innerHTML")
                .await,
            "<code>rg -w '[A-Z]+_SUSPEND'</code>"
        );
        assert_eq!(
            page.text("document.querySelector('#card div').textContent")
                .await,
            "USER GUIDE"
        );
        assert_eq!(
            page.text("document.querySelector('nav li a').textContent")
                .await,
            "CODE"
        );
        assert_eq!(
            page.text("document.querySelector('#ja').innerHTML").await,
            "このツールはとても速いです。"
        );
        assert_eq!(
            page.text("document.querySelector('#held').innerHTML").await,
            "Do not translate this sentence."
        );

        assert_eq!(
            page.text("document.querySelector('#plain pre').textContent")
                .await,
            "# INSTALL WITH CARGO FROM CRATES.IO\n$ cargo install ripgrep\n"
        );
        assert_eq!(
            page.text("document.querySelector('#shell pre').textContent")
                .await,
            "bat README.md  # SHOW A FILE WITH SYNTAX HIGHLIGHTING\n\
             bat -n src/main.rs  # ONLY SHOW LINE NUMBERS\n\
             # INSTALL THE SHELL COMPLETIONS INTO YOUR\n\
             # PROFILE SO THAT THEY LOAD ON EVERY START.\n\
             rg --generate complete-bash > rg.bash"
        );
        assert_eq!(
            page.text("document.querySelector('#rust pre').textContent")
                .await,
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

        for (selector, attribute, expected) in [
            ("#attrs abbr", "title", "REGULAR EXPRESSION"),
            ("#attrs img", "alt", "A SCREENSHOT OF SEARCH RESULTS"),
            ("#attrs button", "aria-label", "COPY TO CLIPBOARD"),
            ("#search", "placeholder", "TYPE TO SEARCH"),
            ("#search", "aria-label", "SEARCH CODE"),
            ("#badges img", "alt", "BUILD STATUS"),
        ] {
            assert_eq!(
                page.text(&format!(
                    "document.querySelector('{selector}').getAttribute('{attribute}')"
                ))
                .await,
                expected,
                "{selector}[{attribute}]"
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
        page.until("document.title", title).await;
        page.until(BODY, original).await;
    });
}

const FIRST_FILE: &str = "Array.from(document.querySelectorAll('.react-code-file-contents:not(#virtual) .react-file-line'), (line) => line.textContent)";
const SECOND_FILE: &str = "Array.from(document.querySelectorAll('#virtual .react-file-line'), (line) => Array.from(line.querySelectorAll('[data-code-text]'), (span) => span.getAttribute('data-code-text')).join(''))";

#[test]
fn code_view_translates_comments_only() {
    scenario(|page| async move {
        page.loaded(BLOB).await;
        let original = page.text(BODY).await;
        let lines = page
            .text("document.querySelector('.react-line-numbers').textContent")
            .await;
        let raw = page.text("document.querySelector('textarea').value").await;
        let virtual_lines = page
            .text("document.querySelector('#virtual .react-code-lines').innerHTML")
            .await;

        page.translate(1, "replace", "comments");
        page.until(
            "document.querySelector('#LC2').textContent",
            "THE MAIN ENTRY POINT INTO RIPGREP.",
        )
        .await;
        page.until(
            &format!("{SECOND_FILE}[0]"),
            "// PRINT THE VERSION AND EXIT.",
        )
        .await;
        page.quiet().await;

        assert_eq!(
            page.eval(FIRST_FILE).await,
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
        let translated_second = json!([
            "// PRINT THE VERSION AND EXIT.",
            "fn version() {",
            "    println!(\"{}\", VERSION); // GOES TO STANDARD OUTPUT",
            "}",
        ]);
        assert_eq!(page.eval(SECOND_FILE).await, translated_second);
        assert_eq!(
            page.text("document.querySelector('.react-line-numbers').textContent")
                .await,
            lines
        );
        assert_eq!(
            page.text("document.querySelector('textarea').value").await,
            raw
        );
        assert!(!page.requested().contains("jemallocator::Jemalloc"));

        let batches = page.batches();
        page.eval(&format!(
            "document.querySelector('#virtual .react-code-lines').innerHTML = {}; true",
            Value::String(virtual_lines)
        ))
        .await;
        page.until(SECOND_FILE, translated_second).await;
        assert_eq!(
            page.batches(),
            batches,
            "a re-rendered file is translated from memory"
        );

        page.restore();
        page.until(BODY, original).await;
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
        let before = page.host.borrow().pages.len();

        page.eval("history.pushState({}, '', '/spa-route'); true")
            .await;

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
            "document.querySelector('#closing').textContent",
            "THAT IS ALL YOU NEED TO GET STARTED WITH THE LIBRARY.",
        )
        .await;
        page.eval("window.rikkaRemembered = true; true").await;

        page.loaded("/elsewhere").await;
        page.until(
            "document.querySelector('p').textContent",
            "ANOTHER PAGE ENTIRELY.",
        )
        .await;
        page.restore();
        page.until(
            "document.querySelector('p').textContent",
            "Another page entirely.",
        )
        .await;
        let reports = page.host.borrow().pages.len();

        page.view.go_back();
        page.at(DOCS).await;
        assert_eq!(
            page.eval("window.rikkaRemembered === true").await,
            json!(true),
            "the page came back from the cache"
        );
        page.until(
            "document.querySelector('#closing').textContent",
            "That is all you need to get started with the library.",
        )
        .await;
        assert!(
            page.host.borrow().pages.len() > reports,
            "the restored page reports itself again"
        );

        page.translate(8, "replace", "comments");
        page.until(
            "document.querySelector('#closing').textContent",
            "THAT IS ALL YOU NEED TO GET STARTED WITH THE LIBRARY.",
        )
        .await;
    });
}

#[test]
fn frames_receive_the_translator_when_translation_starts() {
    scenario(|page| async move {
        page.loaded(FRAMED).await;
        assert_eq!(page.translators(), 0);

        page.translate(4, "replace", "comments");
        page.until(
            "document.querySelector('#outer').textContent",
            "TEXT OUTSIDE THE FRAME.",
        )
        .await;
        let frame = "document.querySelector('#frame').contentDocument";
        page.until_in(
            frame,
            &format!("{frame}.querySelector('#inner').textContent"),
            "TEXT INSIDE THE FRAME.",
        )
        .await;
        assert_eq!(page.translators(), 2, "the top frame and the inner frame");

        page.restore();
        page.until_in(
            frame,
            &format!("{frame}.querySelector('#inner').textContent"),
            "Text inside the frame.",
        )
        .await;
    });
}

#[test]
fn translation_runs_under_a_strict_content_security_policy() {
    scenario(|page| async move {
        page.loaded(STRICT).await;

        page.translate(2, "replace", "comments");
        page.until(
            "document.querySelector('#strict').textContent",
            "THIS PAGE FORBIDS EVERY SCRIPT OF ITS OWN.",
        )
        .await;
    });
}
