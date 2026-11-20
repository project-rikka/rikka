use std::time::Duration;

use rikka_gecko_protocol::{EmbedderMessage, Target};
use rikka_web::{
    CloseReason, Error, Event, Find, Load, Session, SessionSettings, Trust, UserContent, UserScript,
};
use serde_json::json;

use crate::fake::FakeGecko;
use crate::support::{block_on, next_matching};

#[test]
fn window_subscribed_before_open() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");

    let listen = gecko.receive_until(|message| match message {
        EmbedderMessage::Listen(listen) if listen.target != Target::Runtime => Some(listen),
        EmbedderMessage::Open(_) => panic!("the window opened before it was listened to"),
        _ => None,
    });
    let window = gecko.opened();
    assert_eq!(listen.target, Target::Window(window.clone()));
    assert_eq!(window.as_str(), session.id());
    assert!(
        listen
            .events
            .iter()
            .any(|event| event == "GeckoView:PageStart")
    );
    assert!(
        !listen
            .events
            .iter()
            .any(|event| event == "GeckoView:OnLoadRequest")
    );
}

#[test]
fn ephemeral_runtime_opens_private_windows() {
    let (runtime, mut gecko) = FakeGecko::started();
    let _session = runtime
        .session(SessionSettings::new().handle_load_requests(true))
        .expect("a session opens");
    let open = gecko.receive_until(|message| match message {
        EmbedderMessage::Open(open) => Some(open),
        _ => None,
    });
    assert!(open.private);
    assert_eq!(open.init["settings"]["usePrivateMode"], json!(true));
    assert_eq!(open.init["modules"]["GeckoViewNavigation"], json!(true));
    assert_eq!(open.init["modules"]["GeckoViewScroll"], json!(false));
}

#[test]
fn window_commands_wait_for_ready_in_order() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();

    session.set_user_content(UserContent::new().script(UserScript::new("probe", "1")));
    session.load(Load::uri("https://rikka.test/"));
    let early = gecko.quiet_for(Duration::from_millis(300));
    assert!(
        early.iter().all(|message| !matches!(message,
            EmbedderMessage::Event(event) if event.target != Target::Runtime)),
        "nothing reaches the window before it is ready: {early:?}"
    );

    gecko.ready(&window);
    let first = gecko.receive_until(|message| match message {
        EmbedderMessage::Event(event) if event.target != Target::Runtime => Some(event),
        _ => None,
    });
    assert_eq!(first.event, "Rikka:UserContent:Set");
    assert_eq!(first.target, Target::Window(window.clone()));
    let second = gecko.receive_until(|message| match message {
        EmbedderMessage::Event(event) => Some(event),
        _ => None,
    });
    assert_eq!(second.event, "GeckoView:LoadUri");
    assert_eq!(
        second.data.expect("a load")["uri"],
        json!("https://rikka.test/")
    );
}

#[test]
fn page_state_follows_engine() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    let window = Target::Window(gecko.opened());

    gecko.event(
        window.clone(),
        "GeckoView:LocationChange",
        json!({"uri": "https://rikka.test/", "canGoBack": true, "canGoForward": false,
               "isTopLevel": true, "hasUserGesture": false}),
        None,
    );
    gecko.event(
        window.clone(),
        "GeckoView:PageTitleChanged",
        json!({"title": "Rikka"}),
        None,
    );
    gecko.event(
        window,
        "GeckoView:ProgressChanged",
        json!({"progress": 50}),
        None,
    );

    next_matching(&mut events, |event| {
        let page = session.page();
        (matches!(event, Event::Changed) && page.title == "Rikka" && page.progress == 0.5)
            .then_some(())
    });
    let page = session.page();
    assert_eq!(page.uri, "https://rikka.test/");
    assert!(page.can_go_back);
}

#[test]
fn closed_window_ends_session_and_calls() {
    for closed_by_app in [false, true] {
        let (runtime, mut gecko) = FakeGecko::started();
        let session = runtime
            .session(SessionSettings::new())
            .expect("a session opens");
        let mut events = session.events().expect("the first to ask takes the events");
        let window = gecko.opened();
        gecko.ready(&window);

        let finding = std::thread::spawn({
            let session = session.clone();
            move || block_on(session.find(Find::text("rikka")))
        });
        gecko.next_event("GeckoView:FindInPage");
        if closed_by_app {
            session.close();
            let closed = gecko.receive_until(|message| match message {
                EmbedderMessage::Close(close) => Some(close.window),
                _ => None,
            });
            assert_eq!(closed, window);
        } else {
            gecko.closed(&window);
        }

        assert!(matches!(
            finding.join().expect("the find thread"),
            Err(Error::Closed)
        ));
        let reason = next_matching(&mut events, |event| match event {
            Event::Closed(reason) => Some(reason),
            _ => None,
        });
        assert_eq!(reason, CloseReason::Requested);
        assert!(!session.is_open());
    }
}

#[test]
fn last_handle_drop_closes_window() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();
    let second_handle = session.clone();
    drop(session);
    assert!(gecko.quiet_for(Duration::from_millis(200)).is_empty());
    drop(second_handle);
    let closed = gecko.receive_until(|message| match message {
        EmbedderMessage::Close(close) => Some(close.window),
        _ => None,
    });
    assert_eq!(closed, window);
}

#[test]
fn windows_open_after_runtime_setup() {
    let (runtime, mut gecko) = FakeGecko::start();
    let _session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let early = gecko.quiet_for(Duration::from_millis(200));
    assert!(
        early.iter().all(|message| matches!(message,
            EmbedderMessage::Listen(listen) if listen.target == Target::Runtime)),
        "only the runtime's listen goes out before hello: {early:?}"
    );

    gecko.hello();
    let order: Vec<&'static str> = std::iter::from_fn(|| {
        Some(match gecko.receive() {
            EmbedderMessage::Event(event) if event.event == "GeckoView:SetDefaultPrefs" => {
                "preferences"
            }
            EmbedderMessage::Listen(listen) if listen.target != Target::Runtime => "listen",
            EmbedderMessage::Open(_) => "open",
            _ => "other",
        })
    })
    .filter(|step| *step != "other")
    .take(3)
    .collect();
    assert_eq!(order, ["preferences", "listen", "open"]);
}

#[test]
fn initial_blank_page_hidden_until_load() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    let window = gecko.opened();
    gecko.ready(&window);
    let target = Target::Window(window);
    let blank = || {
        [
            ("GeckoView:PageStart", json!({"uri": "about:blank"})),
            (
                "GeckoView:LocationChange",
                json!({"uri": "about:blank", "canGoBack": false, "canGoForward": false,
                       "isTopLevel": true, "hasUserGesture": false}),
            ),
            ("GeckoView:ProgressChanged", json!({"progress": 100})),
            ("GeckoView:PageStop", json!({"success": true})),
        ]
    };

    for (event, data) in blank() {
        gecko.event(target.clone(), event, data, None);
    }
    session.load(Load::uri("https://rikka.test/"));
    for (event, data) in blank() {
        gecko.event(target.clone(), event, data, None);
    }
    for (event, data) in [
        ("GeckoView:PageStart", json!({"uri": "https://rikka.test/"})),
        (
            "GeckoView:LocationChange",
            json!({"uri": "https://rikka.test/", "canGoBack": false, "canGoForward": false,
                   "isTopLevel": true, "hasUserGesture": false}),
        ),
        ("GeckoView:PageStop", json!({"success": true})),
    ] {
        gecko.event(target.clone(), event, data, None);
    }

    let mut seen = Vec::new();
    loop {
        match crate::support::next(&mut events) {
            Event::Changed => assert_ne!(session.page().uri, "about:blank"),
            Event::PageStarted(uri) => seen.push(format!("started {uri}")),
            Event::PageStopped { success } => {
                seen.push(format!("stopped {success}"));
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(seen, ["started https://rikka.test/", "stopped true"]);
    assert_eq!(session.page().uri, "https://rikka.test/");
}

#[test]
fn failed_certificate_can_be_trusted() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    let window = gecko.opened();
    gecko.ready(&window);
    let target = Target::Window(window);

    gecko.event(
        target.clone(),
        "Rikka:View:CertificateError",
        json!({"uri": "https://self-signed.test:8443/", "host": "self-signed.test", "port": 8443,
               "certificate": "AQID", "overridable": true}),
        None,
    );
    gecko.event(
        target,
        "GeckoView:OnLoadError",
        json!({"uri": "https://self-signed.test:8443/", "error": 2153390067u32,
               "errorModule": 21, "errorClass": 2}),
        Some(3),
    );
    let error = next_matching(&mut events, |event| match event {
        Event::LoadError(error) => Some(error),
        _ => None,
    });
    assert_eq!(error.class, Some(rikka_web::ErrorClass::BadCertificate));
    let certificate = error
        .certificate
        .clone()
        .expect("the certificate came along");
    assert_eq!(
        (
            certificate.host.as_str(),
            certificate.port,
            certificate.der.as_slice()
        ),
        ("self-signed.test", 8443, [1, 2, 3].as_slice())
    );
    error.show_nothing();

    let trusting = std::thread::spawn({
        let session = session.clone();
        move || block_on(session.allow_certificate(&certificate, Trust::Temporary))
    });
    let event = gecko.next_event("Rikka:View:AllowCertificate");
    assert_eq!(
        event.data.expect("an exception"),
        json!({"host": "self-signed.test", "port": 8443, "certificate": "AQID", "permanent": false})
    );
    gecko.reply(event.callback.expect("a call"), true, json!(null));
    trusting
        .join()
        .expect("the trusting thread")
        .expect("the exception is kept");
    gecko.next_event("GeckoView:Reload");
}

type Command = Box<dyn Fn(&Session)>;

#[test]
fn session_commands_reach_engine_by_name() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();
    gecko.ready(&window);
    let opener = session.clone();
    let cases: Vec<(Command, &str, serde_json::Value)> = vec![
        (
            Box::new(Session::purge_history),
            "GeckoView:PurgeHistory",
            json!(null),
        ),
        (
            Box::new(|session| session.set_high_priority(true)),
            "GeckoView:SetPriorityHint",
            json!({"priorityHint": 1}),
        ),
        (
            Box::new(|session| session.set_high_priority(false)),
            "GeckoView:SetPriorityHint",
            json!({"priorityHint": 0}),
        ),
        (
            Box::new(Session::stop_capture),
            "Rikka:View:StopCapture",
            json!({"onlyScreen": false}),
        ),
        (
            Box::new(Session::stop_screen_sharing),
            "Rikka:View:StopCapture",
            json!({"onlyScreen": true}),
        ),
        (
            Box::new(|session| session.replace_misspelling("colour")),
            "Rikka:View:ReplaceMisspelling",
            json!({"suggestion": "colour"}),
        ),
        (
            Box::new(Session::add_to_dictionary),
            "Rikka:View:AddToDictionary",
            json!(null),
        ),
        (
            Box::new(move |session| {
                session.load(Load::uri("https://rikka.test/next").referrer_session(&opener));
            }),
            "GeckoView:LoadUri",
            json!({"uri": "https://rikka.test/next", "referrerSessionId": window.as_str()}),
        ),
    ];
    for (command, name, expected) in cases {
        command(&session);
        let event = gecko.receive_until(|message| match message {
            EmbedderMessage::Event(event) if event.target != Target::Runtime => Some(event),
            _ => None,
        });
        assert_eq!(event.event, name);
        let data = event.data.unwrap_or(json!(null));
        match expected.as_object() {
            Some(fields) => {
                for (key, value) in fields {
                    assert_eq!(&data[key], value, "{name} sends {key}");
                }
            }
            None => assert!(data.is_null() || data == json!({}), "{name} sends {data}"),
        }
    }
}

#[test]
fn find_options_reach_engine() {
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();
    gecko.ready(&window);
    let cases = [
        (
            Find::text("rikka").match_case().whole_word(),
            json!({"searchString": "rikka", "backwards": false, "linksOnly": false,
                   "matchCase": true, "wholeWord": true, "matchDiacritics": false}),
        ),
        (
            Find::previous().links_only().match_diacritics(),
            json!({"searchString": null, "backwards": true, "linksOnly": true,
                   "matchCase": false, "wholeWord": false, "matchDiacritics": true}),
        ),
    ];
    for (find, expected) in cases {
        let finding = std::thread::spawn({
            let session = session.clone();
            move || block_on(session.find(find))
        });
        let event = gecko.next_event("GeckoView:FindInPage");
        assert_eq!(event.data.expect("a find"), expected);
        gecko.reply(
            event.callback.expect("a call"),
            true,
            json!({"found": false, "wrapped": false, "current": 0, "total": 0,
                   "searchString": "rikka", "flags": {}}),
        );
        finding
            .join()
            .expect("the find thread")
            .expect("the find settles");
    }
}
