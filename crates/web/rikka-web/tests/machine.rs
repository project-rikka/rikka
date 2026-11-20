mod support;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use futures_channel::mpsc::UnboundedReceiver;
use rikka_input::evdev;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::{Element, Event as UiEvent, Rendering, State, Styled, TypedUi};
use rikka_web::{
    ChosenWith, DownloadState, DropdownItem, EditCommand, Event, InjectAt, Load, MessageHandler,
    PageState, Popup, Preferences, Prompt, Runtime, RuntimeEvent, Session, SessionSettings,
    UserContent, UserScript, World, web_view,
};
use serde_json::json;

use crate::support::{block_on, next, next_matching, next_matching_within};

const STARTUP: Duration = Duration::from_secs(45);
const FRAME: Duration = Duration::from_millis(16);
const SETTLE: Duration = Duration::from_secs(1);

struct Engine {
    runtime: Option<Runtime>,
    events: UnboundedReceiver<RuntimeEvent>,
}

impl std::ops::Deref for Engine {
    type Target = Runtime;

    fn deref(&self) -> &Runtime {
        self.runtime
            .as_ref()
            .expect("the engine lives until it is dropped")
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.runtime.take();
        rikka_web::wait_for_closed_runtimes();
    }
}

fn opened() -> (Engine, Session, UnboundedReceiver<Event>) {
    opened_with(Preferences::new(), SessionSettings::new())
}

fn opened_with(
    preferences: Preferences,
    settings: SessionSettings,
) -> (Engine, Session, UnboundedReceiver<Event>) {
    let runtime = Runtime::builder()
        .preferences(preferences)
        .start()
        .expect("the installed engine starts");
    let mut runtime_events = runtime.events().expect("the first to ask takes the events");
    next_matching_within(STARTUP, &mut runtime_events, |event| match event {
        RuntimeEvent::Started { .. } => Some(()),
        RuntimeEvent::Exited(exit) => panic!("the engine exited: {exit:?}"),
        _ => None,
    });
    let session = runtime.session(settings).expect("a session opens");
    let events = session.events().expect("the first to ask takes the events");
    let engine = Engine {
        runtime: Some(runtime),
        events: runtime_events,
    };
    (engine, session, events)
}

fn loaded(session: &Session, events: &mut UnboundedReceiver<Event>, load: Load) {
    session.load(load);
    let success = next_matching(events, |event| match event {
        Event::PageStopped { success } => Some(success),
        _ => None,
    });
    assert!(success, "the page loads");
}

fn served(response: impl AsRef<[u8]> + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let address = listener.local_addr().expect("a local address");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                line.clear();
            }
            let _ = stream.write_all(response.as_ref());
        }
    });
    format!("http://{address}/")
}

fn until_page(
    session: &Session,
    events: &mut UnboundedReceiver<Event>,
    settled: impl Fn(&PageState) -> bool,
) {
    while !settled(&session.page()) {
        if let Event::Closed(reason) = next(events) {
            panic!("the session went away: {reason:?}");
        }
    }
}

#[test]
fn page_loads_and_runs_scripts_in_both_worlds() {
    let (_runtime, session, mut events) = opened();
    session.load(Load::html("<title>Rikka</title><p id=greeting>hello</p>"));
    let success = next_matching(&mut events, |event| match event {
        Event::PageStopped { success } => Some(success),
        _ => None,
    });
    assert!(success, "the page loads");
    until_page(&session, &mut events, |page| page.title == "Rikka");

    let sum = block_on(session.evaluate("1 + 2", &World::Page)).expect("a page script runs");
    assert_eq!(sum, json!(3));
    let text = block_on(session.evaluate(
        "document.getElementById('greeting').textContent",
        &World::isolated("rikka-machine"),
    ))
    .expect("an isolated script reads the page");
    assert_eq!(text, json!("hello"));
}

#[test]
fn replying_handler_answers_page() {
    let (_runtime, session, mut events) = opened();
    session.set_user_content(
        UserContent::new()
            .handler(MessageHandler::new("probe").with_replies())
            .script(
                UserScript::new(
                    "ask",
                    "window.rikka.messageHandlers.probe.postMessage({n: 2}).then(answer => { document.title = String(answer.doubled); });",
                )
                .inject_at(InjectAt::End),
            ),
    );
    session.load(Load::html("<p>probe</p>"));
    let message = next_matching(&mut events, |event| match event {
        Event::ScriptMessage(message) => Some(message),
        _ => None,
    });
    assert_eq!(message.name, "probe");
    assert_eq!(message.body, json!({"n": 2}));
    message
        .reply
        .expect("the handler replies")
        .send(json!({"doubled": 4}));
    until_page(&session, &mut events, |page| page.title == "4");
}

#[test]
fn sized_session_has_stable_viewport() {
    let (_runtime, session, mut events) =
        opened_with(Preferences::new(), SessionSettings::new().size(1280, 1600));
    loaded(&session, &mut events, Load::html("<p>viewport</p>"));
    let size = block_on(session.evaluate(
        "[outerWidth, outerHeight, innerWidth, innerHeight]",
        &World::Page,
    ))
    .expect("a page script runs");
    assert_eq!(size, json!([1280, 1600, 1280, 1600]));
}

#[test]
fn window_open_distinguishes_popups_and_tabs() {
    let preferences = Preferences::new().set("dom.disable_open_during_load", false);
    let (_runtime, session, mut events) = opened_with(preferences, SessionSettings::new());
    loaded(&session, &mut events, Load::html("<p>opener</p>"));
    for (features, popup) in [
        (
            "popup,width=400,height=300",
            Some(Popup {
                width: Some(400),
                height: Some(300),
            }),
        ),
        ("", None),
    ] {
        let script = format!(
            "setTimeout(() => window.open('https://rikka.test/opened', '_blank', '{features}')); true"
        );
        block_on(session.evaluate(&script, &World::Page)).expect("a page script runs");
        let request = next_matching(&mut events, |event| match event {
            Event::NewSession(request) => Some(request),
            _ => None,
        });
        assert_eq!(request.popup, popup, "{features}");
        request.deny();
    }
}

#[test]
fn permit_unload_runs_beforeunload() {
    let preferences =
        Preferences::new().set("dom.require_user_interaction_for_beforeunload", false);
    let (_runtime, session, mut events) = opened_with(preferences, SessionSettings::new());
    loaded(
        &session,
        &mut events,
        Load::html("<script>onbeforeunload = event => event.preventDefault();</script>"),
    );
    let asking = std::thread::spawn({
        let session = session.clone();
        move || block_on(session.permit_unload())
    });
    let prompt = next_matching(&mut events, |event| match event {
        Event::Prompt(Prompt::BeforeUnload(prompt)) => Some(prompt),
        _ => None,
    });
    drop(prompt);
    let permitted = asking
        .join()
        .expect("the asking thread")
        .expect("the engine answers");
    assert!(!permitted, "a dismissed prompt keeps the page");
}

#[test]
fn cookies_include_http_only_ones() {
    let page = served(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: sid=secret; HttpOnly; Path=/\r\nSet-Cookie: theme=dark; Path=/\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
    );
    let (runtime, session, mut events) = opened();
    loaded(&session, &mut events, Load::uri(page.clone()));
    let cookies = block_on(runtime.storage().cookies(page, &session.partition()))
        .expect("the cookies are read");
    let sid = cookies
        .iter()
        .find(|cookie| cookie.name == "sid")
        .expect("the HttpOnly cookie is listed");
    assert!(sid.http_only);
    assert_eq!(sid.value, "secret");
    assert!(cookies.iter().any(|cookie| cookie.name == "theme"));
}

#[test]
fn page_notification_reaches_the_runtime() {
    let page = served(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 9\r\nConnection: close\r\n\r\n<p>notify",
    );
    let preferences = Preferences::new().set("permissions.default.desktop-notification", 1);
    let (mut engine, session, mut events) = opened_with(preferences, SessionSettings::new());
    loaded(&session, &mut events, Load::uri(page));
    block_on(session.evaluate(
        "new Notification('Rikka', { body: 'hello' }); true",
        &World::Page,
    ))
    .expect("the script runs");
    let notification = next_matching(&mut engine.events, |event| match event {
        RuntimeEvent::Notification(notification) => Some(notification),
        _ => None,
    });
    assert_eq!(notification.title, "Rikka");
    assert_eq!(notification.text, "hello");
}

#[test]
fn save_link_prompts_and_writes_file() {
    let page = served(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\nConnection: close\r\n\r\nrikk",
    );
    let target = std::env::temp_dir().join(format!("rikka-web-save-{}", std::process::id()));
    let (mut engine, session, mut events) = opened();
    loaded(&session, &mut events, Load::html("<p>save</p>"));
    let saving = std::thread::spawn({
        let session = session.clone();
        let file = format!("{page}file.bin");
        move || block_on(session.download(file))
    });
    let request = next_matching(&mut events, |event| match event {
        Event::Download(request) => Some(request),
        _ => None,
    });
    assert_eq!(request.reason, rikka_web::DownloadReason::Save);
    assert_eq!(request.filename, "file.bin");
    request.save_to(&target).expect("an absolute path");
    saving
        .join()
        .expect("the saving thread")
        .expect("the download starts");
    next_matching(&mut engine.events, |event| match event {
        RuntimeEvent::Download(download)
            if download.path == target && download.state == DownloadState::Finished =>
        {
            Some(())
        }
        _ => None,
    });
    assert_eq!(
        std::fs::read(&target).expect("the saved file"),
        b"rikk",
        "the saved file holds the body"
    );
    std::fs::remove_file(&target).expect("the saved file");
}

#[test]
fn navigating_to_attachment_downloads_it() {
    const LENGTH: usize = 4 << 20;
    let body: Vec<u8> = (0..LENGTH).map(|index| (index % 251) as u8).collect();
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Disposition: attachment; filename=\"archive.zip\"\r\nContent-Length: {LENGTH}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes();
    response.extend_from_slice(&body);
    let page = served(response);
    let target =
        std::env::temp_dir().join(format!("rikka-web-attachment-{}.zip", std::process::id()));
    let (mut engine, session, mut events) = opened();
    loaded(&session, &mut events, Load::html("<p>download</p>"));
    session.load(Load::uri(format!("{page}archive.zip")));
    let request = next_matching(&mut events, |event| match event {
        Event::Download(request) => Some(request),
        Event::Prompt(Prompt::Alert(alert)) => panic!("the engine alerted: {:?}", alert.message),
        _ => None,
    });
    assert_eq!(request.filename, "archive.zip");
    request.save_to(&target).expect("an absolute path");
    let download = next_matching(&mut engine.events, |event| match event {
        RuntimeEvent::Download(download)
            if download.path == target
                && matches!(
                    download.state,
                    DownloadState::Finished | DownloadState::Failed
                ) =>
        {
            Some(download)
        }
        _ => None,
    });
    assert_eq!(
        download.state,
        DownloadState::Finished,
        "{:?}",
        download.error
    );
    assert_eq!(std::fs::read(&target).expect("the saved file"), body);
    std::fs::remove_file(&target).expect("the saved file");
}

#[test]
fn edit_commands_reach_the_focused_field() {
    let (_runtime, session, mut events) = opened();
    loaded(
        &session,
        &mut events,
        Load::html("<input id=field value=rikka autofocus>"),
    );
    session.set_focused(true);
    block_on(session.evaluate("document.getElementById('field').focus()", &World::Page))
        .expect("the field takes focus");
    for command in [EditCommand::SelectAll, EditCommand::Delete] {
        block_on(session.edit(command)).expect("the engine runs the command");
    }
    let value = block_on(session.evaluate("document.getElementById('field').value", &World::Page))
        .expect("a page script runs");
    assert_eq!(value, json!(""));
}

const SELECT_AT: Point = Point::new(60.0, 27.0);

const SELECT_PAGE: &str = r#"<title>ready</title>
<select id=s style="position:fixed;left:10px;top:12px;width:200px;height:30px">
  <option>English</option><option>Deutsch</option><option>Nihongo</option>
</select>
<script>
  const seen = [];
  const note = event => {
    if (event.type === "input" || event.type === "change" || event.target.tagName === "OPTION") {
      seen.push(event.type);
      document.title = seen.join(" ") + " " + s.selectedIndex;
    }
  };
  for (const type of ["mousedown", "mouseup", "click", "input", "change"]) {
    s.addEventListener(type, note);
  }
</script>"#;

#[derive(State)]
struct Shown {
    session: Session,
}

fn shown(shown: &Shown) -> Element {
    Element::new(web_view(&shown.session).fill())
}

fn press(ui: &mut TypedUi<Shown>, at: Point) {
    let (button, modifiers, position) = (PointerButton::Primary, Modifiers::NONE, at);
    ui.handle_event(UiEvent::PointerMoved(at));
    ui.handle_event(UiEvent::PointerPressed {
        position,
        button,
        modifiers,
    });
    ui.handle_event(UiEvent::PointerReleased {
        position,
        button,
        modifiers,
    });
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
}

fn dropdown(events: &mut UnboundedReceiver<Event>) -> rikka_web::Dropdown {
    next_matching(events, |event| match event {
        Event::Dropdown(dropdown) => Some(dropdown),
        Event::Prompt(Prompt::Choose(_)) => panic!("a second picker opened for the select"),
        _ => None,
    })
}

#[test]
fn select_opens_one_anchored_dropdown_and_fires_firefox_events() {
    let (_runtime, session, mut events) = opened_with(
        Preferences::new()
            .set("dom.input_events.security.minNumTicks", 0)
            .set("dom.input_events.security.minTimeElapsedInMS", 0),
        SessionSettings::new(),
    );
    loaded(&session, &mut events, Load::html(SELECT_PAGE));
    session.set_visible(true);
    let mut ui = TypedUi::new(
        Shown {
            session: session.clone(),
        },
        shown,
        Theme::dark(),
    );
    ui.set_viewport(Size::new(640.0, 480.0), 1.0);
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));

    let cases = [
        (
            ChosenWith::Pointer,
            2,
            "mousedown mouseup input change click 2",
        ),
        (ChosenWith::Keyboard, 1, "input change 1"),
    ];
    for (with, index, title) in cases {
        block_on(session.evaluate("seen.length = 0", &World::Page)).expect("a page script runs");
        press(&mut ui, SELECT_AT);
        let opened = dropdown(&mut events);
        let anchor = opened.anchor;
        assert!(
            (anchor.x() - 10.0).abs() < 1.0
                && (anchor.y() - 12.0).abs() < 1.0
                && (anchor.width() - 200.0).abs() < 1.0
                && (anchor.height() - 30.0).abs() < 1.0,
            "{anchor:?}"
        );
        let labels: Vec<&str> = opened
            .items
            .iter()
            .map(|item| match item {
                DropdownItem::Option(option) => option.label.as_str(),
                other => panic!("only options were listed: {other:?}"),
            })
            .collect();
        assert_eq!(labels, ["English", "Deutsch", "Nihongo"]);
        opened.choose(index, with);
        until_page(&session, &mut events, |page| page.title == title);
    }

    press(&mut ui, SELECT_AT);
    let opened = dropdown(&mut events);
    block_on(session.evaluate("s.blur()", &World::Page)).expect("a page script runs");
    let hidden = next_matching(&mut events, |event| match event {
        Event::DropdownHidden(id) => Some(id),
        _ => None,
    });
    assert_eq!(hidden, opened.id);
}

fn rendered_for(ui: &mut TypedUi<Shown>, duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
        std::thread::sleep(FRAME);
    }
}

fn key(ui: &mut TypedUi<Shown>, key: Key, code: KeyCode, modifiers: Modifiers, pressed: bool) {
    let location = KeyLocation::Standard;
    ui.handle_event(match pressed {
        true => UiEvent::KeyPressed {
            key,
            code,
            location,
            modifiers,
        },
        false => UiEvent::KeyReleased {
            key,
            code,
            location,
            modifiers,
        },
    });
}

#[test]
fn copying_keeps_the_engine_running() {
    let (_runtime, session, mut events) = opened_with(
        Preferences::new()
            .set("dom.input_events.security.minNumTicks", 0)
            .set("dom.input_events.security.minTimeElapsedInMS", 0),
        SessionSettings::new(),
    );
    loaded(
        &session,
        &mut events,
        Load::html("<p id=copied style='font-size:32px'>copy this text</p>"),
    );
    session.set_visible(true);
    let mut ui = TypedUi::new(
        Shown {
            session: session.clone(),
        },
        shown,
        Theme::dark(),
    );
    ui.set_viewport(Size::new(640.0, 480.0), 1.0);
    rendered_for(&mut ui, SETTLE);
    press(&mut ui, Point::new(60.0, 30.0));
    block_on(session.evaluate(
        "getSelection().selectAllChildren(document.getElementById('copied'))",
        &World::Page,
    ))
    .expect("a page script selects the text");

    let control = KeyCode::from_evdev(evdev::KEY_LEFTCTRL);
    let c = KeyCode::from_evdev(evdev::key_code(Key::Char('c')).expect("c has a key code"));
    key(&mut ui, Key::ControlLeft, control, Modifiers::NONE, true);
    key(&mut ui, Key::Char('c'), c, Modifiers::CONTROL, true);
    key(&mut ui, Key::Char('c'), c, Modifiers::CONTROL, false);
    key(
        &mut ui,
        Key::ControlLeft,
        control,
        Modifiers::CONTROL,
        false,
    );
    rendered_for(&mut ui, SETTLE);
    block_on(session.edit(EditCommand::Copy)).expect("the engine copies from its menu command");
    rendered_for(&mut ui, SETTLE);

    let alive = block_on(session.evaluate("1 + 1", &World::Page)).expect("the engine still runs");
    assert_eq!(alive, json!(2));
}
