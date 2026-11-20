use std::time::Duration;

use rikka_gecko_protocol::{EmbedderMessage, Target};
use rikka_types::Rect;
use rikka_web::{ChosenWith, DropdownId, Event, Popup, Prompt, SessionSettings};
use serde_json::{Value, json};

use crate::fake::FakeGecko;
use crate::support::{next, next_matching};

struct Case {
    event: &'static str,
    data: Value,
    success: bool,
    answer: Option<Value>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            event: "GeckoView:ContentPermission",
            data: json!({"uri": "https://rikka.test/", "principal": "https://rikka.test",
                         "perm": "geolocation", "value": 0, "privateMode": false}),
            success: true,
            answer: Some(json!(2)),
        },
        Case {
            event: "GeckoView:MediaPermission",
            data: json!({"uri": "https://rikka.test/", "video": [{"type": "videoinput",
                         "id": "c1", "rawId": "c1", "name": "Camera", "mediaSource": "camera"}]}),
            success: true,
            answer: Some(json!(false)),
        },
        Case {
            event: "GeckoView:Prompt",
            data: json!({"prompt": {"id": "p1", "type": "alert", "msg": "hello"}}),
            success: true,
            answer: None,
        },
        Case {
            event: "GeckoView:OnNewSession",
            data: json!({"uri": "https://rikka.test/popup", "newSessionId": "n1"}),
            success: true,
            answer: Some(json!(false)),
        },
        Case {
            event: "Rikka:DownloadRequest",
            data: json!({"url": "https://rikka.test/a.zip", "filename": "a.zip",
                         "mimeType": null, "contentLength": null, "reason": "attachment"}),
            success: true,
            answer: None,
        },
        Case {
            event: "GeckoView:GetVisited",
            data: json!({"urls": ["https://rikka.test/"]}),
            success: true,
            answer: None,
        },
        Case {
            event: "Rikka:UserContent:Message",
            data: json!({"name": "probe", "world": null, "body": {"n": 1},
                         "frame": {"url": "https://rikka.test/", "top": true}}),
            success: false,
            answer: Some(json!("the message was not answered")),
        },
        Case {
            event: "Rikka:View:Dropdown",
            data: dropdown(1),
            success: true,
            answer: None,
        },
    ]
}

fn dropdown(id: u64) -> Value {
    json!({
        "id": id,
        "rect": {"left": 10.0, "top": 20.0, "right": 130.0, "bottom": 44.0},
        "items": [
            {"kind": "option", "index": 0, "label": "English", "disabled": false, "tooltip": null},
            {"kind": "option", "index": 1, "label": "Deutsch", "disabled": false, "tooltip": null},
        ],
        "selected": 0,
    })
}

fn opened(gecko: &mut FakeGecko, runtime: &rikka_web::Runtime) -> (rikka_web::Session, Target) {
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();
    gecko.ready(&window);
    (session, Target::Window(window))
}

#[test]
fn dropped_requests_reply_default_once() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (session, window) = opened(&mut gecko, &runtime);
    let mut events = session.events().expect("the first to ask takes the events");
    for (callback, case) in (1_u64..).zip(cases()) {
        gecko.event(window.clone(), case.event, case.data, Some(callback));
        let request = next_matching(&mut events, |event| match event {
            Event::Changed => None,
            other => Some(other),
        });
        drop(request);
        let reply = gecko.reply_to(callback);
        assert_eq!(reply.success, case.success, "{}", case.event);
        assert_eq!(reply.data, case.answer, "{}", case.event);
    }
    let later = gecko.quiet_for(Duration::from_millis(200));
    assert!(
        later
            .iter()
            .all(|message| !matches!(message, EmbedderMessage::Reply(_))),
        "each request is answered once: {later:?}"
    );
}

#[test]
fn unhandled_requests_reply_immediately() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (_session, window) = opened(&mut gecko, &runtime);
    for (callback, case) in (1_u64..).zip(cases()) {
        gecko.event(window.clone(), case.event, case.data, Some(callback));
        let reply = gecko.reply_to(callback);
        assert_eq!(reply.data, case.answer, "{}", case.event);
    }
}

#[test]
fn reply_overrides_default() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (session, window) = opened(&mut gecko, &runtime);
    let mut events = session.events().expect("the first to ask takes the events");

    gecko.event(
        window.clone(),
        "GeckoView:Prompt",
        json!({"prompt": {"id": "p2", "type": "text", "msg": "name?", "value": "a"}}),
        Some(4),
    );
    let Event::Prompt(Prompt::Text(prompt)) = next(&mut events) else {
        panic!("a text prompt arrives");
    };
    assert_eq!(prompt.value.as_deref(), Some("a"));
    prompt.submit("rikka", None);
    assert_eq!(gecko.reply_to(4).data, Some(json!({"text": "rikka"})));

    gecko.event(
        window,
        "Rikka:UserContent:Message",
        json!({"name": "probe", "world": "rikka-probe", "body": [1, 2],
               "frame": {"url": "https://rikka.test/", "top": false}}),
        Some(5),
    );
    let Event::ScriptMessage(message) = next(&mut events) else {
        panic!("a script message arrives");
    };
    assert_eq!(message.world, rikka_web::World::isolated("rikka-probe"));
    assert!(!message.top_frame);
    message
        .reply
        .expect("a replying handler can be answered")
        .send(json!({"sum": 3}));
    let reply = gecko.reply_to(5);
    assert!(reply.success);
    assert_eq!(reply.data, Some(json!({"sum": 3})));
}

#[test]
fn runtime_answers_unsubscribed_requests() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (_session, window) = opened(&mut gecko, &runtime);

    gecko.event(
        window.clone(),
        "GeckoView:AndroidPermission",
        json!({"perms": ["camera"]}),
        Some(1),
    );
    assert_eq!(gecko.reply_to(1).data, Some(json!(true)));

    gecko.event(window.clone(), "GeckoView:Unknown", json!({}), Some(2));
    assert!(!gecko.reply_to(2).success);

    gecko.event(
        Target::Window(rikka_gecko_protocol::WindowId::new("nobody").expect("an id")),
        "GeckoView:PageStart",
        json!({"uri": "https://rikka.test/"}),
        Some(3),
    );
    assert!(!gecko.reply_to(3).success);
}

#[test]
fn new_session_opens_before_reply() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (session, window) = opened(&mut gecko, &runtime);
    let mut events = session.events().expect("the first to ask takes the events");

    gecko.event(
        window,
        "GeckoView:OnNewSession",
        json!({"uri": "https://rikka.test/popup", "newSessionId": "n7"}),
        Some(9),
    );
    let Event::NewSession(request) = next(&mut events) else {
        panic!("a new session request arrives");
    };
    let popup = request.open().expect("the popup opens");
    assert_eq!(popup.id(), "n7");

    let order: Vec<&'static str> = std::iter::from_fn(|| {
        Some(match gecko.receive() {
            EmbedderMessage::Listen(listen) if listen.target != Target::Runtime => "listen",
            EmbedderMessage::Open(open) if open.window.as_str() == "n7" => "open",
            EmbedderMessage::Reply(reply) if reply.callback.get() == 9 => "reply",
            _ => "other",
        })
    })
    .filter(|step| *step != "other")
    .take(3)
    .collect();
    assert_eq!(order, ["listen", "open", "reply"]);
}

#[test]
fn new_session_reports_popup_size() {
    let cases = [
        (json!({}), None),
        (json!({"popup": false, "features": "width=500"}), None),
        (
            json!({"popup": true, "features": ""}),
            Some(Popup::default()),
        ),
        (
            json!({"popup": true, "features": "width=500,height=600"}),
            Some(Popup {
                width: Some(500),
                height: Some(600),
            }),
        ),
        (
            json!({"popup": true, "features": " InnerWidth = 420px ; innerheight=-3,left=10"}),
            Some(Popup {
                width: Some(420),
                height: None,
            }),
        ),
        (
            json!({"popup": true, "features": "popup,height=0 width=640 , height"}),
            Some(Popup {
                width: Some(640),
                height: None,
            }),
        ),
    ];
    let (runtime, mut gecko) = FakeGecko::started();
    let (session, window) = opened(&mut gecko, &runtime);
    let mut events = session.events().expect("the first to ask takes the events");
    for (callback, (extra, popup)) in (20..).zip(cases) {
        let mut data = json!({"uri": "https://rikka.test/popup", "newSessionId": "n7"});
        data.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        gecko.event(
            window.clone(),
            "GeckoView:OnNewSession",
            data,
            Some(callback),
        );
        let Event::NewSession(request) = next(&mut events) else {
            panic!("a new session request arrives");
        };
        assert_eq!(request.popup, popup, "{extra}");
        request.deny();
    }
}

#[test]
fn dropdown_anchors_in_view_coordinates_and_answers_the_choice() {
    let (runtime, mut gecko) = FakeGecko::started();
    let (session, window) = opened(&mut gecko, &runtime);
    let mut events = session.events().expect("the first to ask takes the events");

    gecko.event(window.clone(), "Rikka:View:Dropdown", dropdown(4), Some(6));
    let Event::Dropdown(dropdown_shown) = next(&mut events) else {
        panic!("a dropdown arrives");
    };
    assert_eq!(dropdown_shown.id, DropdownId(4));
    assert_eq!(dropdown_shown.anchor, Rect::new(10.0, 20.0, 120.0, 24.0));
    assert_eq!(dropdown_shown.selected, Some(0));

    gecko.event(
        window.clone(),
        "Rikka:View:DropdownUpdated",
        json!({"id": 4, "items": [], "selected": null}),
        None,
    );
    let Event::DropdownUpdated(update) = next(&mut events) else {
        panic!("the update arrives");
    };
    assert_eq!((update.id, update.selected), (DropdownId(4), None));
    gecko.event(window, "Rikka:View:DropdownHidden", json!({"id": 4}), None);
    let Event::DropdownHidden(hidden) = next(&mut events) else {
        panic!("the hide arrives");
    };
    assert_eq!(hidden, DropdownId(4));

    dropdown_shown.choose(1, ChosenWith::Keyboard);
    let reply = gecko.reply_to(6);
    assert!(reply.success);
    assert_eq!(reply.data, Some(json!({"index": 1, "with": "keyboard"})));
}
