use browser_translate::{
    CodeMode, Command, Display, Error, Limit, Message, Node, PageReport, Request, Unit, Watch,
    detect,
};
use rikka_languages::Language;
use serde_json::{Value, json};

fn translate(units: Value) -> Value {
    json!({"kind": "translate", "session": 7, "units": units})
}

#[test]
fn parses_page_messages() {
    let message = Message::parse(&json!({"kind": "page", "lang": "en-US", "sample": "Hello"}));
    assert_eq!(
        message.expect("page message"),
        Message::Page(PageReport {
            lang: Some("en-US".into()),
            sample: "Hello".into()
        })
    );
    let message = Message::parse(&translate(json!([{
        "pre": false,
        "nodes": ["Click ", {"i": 0, "nodes": ["here"]}, " or ", {"i": 1, "keep": "--help"}]
    }])));
    assert_eq!(
        message.expect("translate request"),
        Message::Translate(Request {
            session: 7,
            units: vec![
                Unit::new(vec![
                    Node::Text("Click ".into()),
                    Node::Element {
                        id: 0,
                        nodes: vec![Node::Text("here".into())]
                    },
                    Node::Text(" or ".into()),
                    Node::Keep {
                        id: 1,
                        text: "--help".into()
                    },
                ])
                .expect("a valid unit")
            ]
        })
    );
}

#[test]
fn rejects_unknown_messages() {
    assert!(matches!(
        Message::parse(&json!({"kind": "explode"})),
        Err(Error::UnknownKind(kind)) if kind == "explode"
    ));
    assert!(matches!(
        Message::parse(&json!([1, 2])),
        Err(Error::Malformed("message"))
    ));
    assert!(matches!(
        Message::parse(&json!({"session": 1})),
        Err(Error::Malformed("kind"))
    ));
    assert!(Message::parse(&json!({"kind": "translate", "session": -1, "units": []})).is_err());
    for unit in [
        json!("text"),
        json!({"pre": false}),
        json!({"pre": "yes", "nodes": []}),
        json!({"nodes": [{"i": 0}]}),
        json!({"nodes": [{"i": -1, "nodes": []}]}),
        json!({"nodes": [{"i": 0, "keep": "x", "nodes": []}]}),
        json!({"nodes": [{"i": 0, "keep": 3}]}),
        json!({"nodes": [7]}),
    ] {
        assert!(
            matches!(
                Message::parse(&translate(json!([unit]))),
                Err(Error::Malformed(_))
            ),
            "{unit}"
        );
    }
}

#[test]
fn translator_travels_only_to_frames_without_it() {
    let watch = |translator: bool| match Message::parse(&json!({
        "kind": "watch",
        "top": false,
        "session": null,
        "display": null,
        "translator": translator,
    })) {
        Ok(Message::Watch(watch)) => watch,
        other => panic!("expected a watch, got {other:?}"),
    };
    let bare = watch(false);
    let loaded = watch(true);
    assert_eq!(
        bare,
        Watch {
            top: false,
            session: None,
            display: None,
            translator: false,
        }
    );
    assert!(matches!(
        Message::parse(&json!({"kind": "watch", "top": true})),
        Err(Error::Malformed("translator"))
    ));

    let translate = Command::Translate {
        session: 3,
        to: Language::lookup("ja").expect("Japanese"),
        display: Display::Replace,
        code: CodeMode::Comments,
    };
    let sent = translate.answer(&bare);
    assert_eq!(sent["do"], "translate");
    assert_eq!(sent["to"], "ja");
    assert!(
        sent["translator"]
            .as_str()
            .is_some_and(|source| source.starts_with("((host) =>"))
    );
    assert!(translate.answer(&loaded).get("translator").is_none());
    assert!(Command::Restore.answer(&bare).get("translator").is_none());
}

#[test]
fn enforces_message_limits() {
    let refused = |message: Value| match Message::parse(&message) {
        Err(Error::TooLarge(limit)) => Some(limit),
        Err(Error::DuplicateId(_)) => None,
        other => panic!("{message:.80} was accepted: {other:?}"),
    };
    let long_sample = json!({"kind": "page", "lang": "", "sample": "a".repeat(100_000)});
    assert_eq!(refused(long_sample), Some(Limit::Sample));
    let units: Vec<Value> = (0..10_000)
        .map(|_| json!({"pre": false, "nodes": []}))
        .collect();
    assert_eq!(refused(translate(json!(units))), Some(Limit::Units));
    let long_unit = json!({"nodes": ["a".repeat(1_000_000)]});
    assert_eq!(
        refused(translate(json!([long_unit]))),
        Some(Limit::UnitText)
    );
    let mut deep = json!("x");
    for id in 0..200 {
        deep = json!({"i": id, "nodes": [deep]});
    }
    assert_eq!(
        refused(translate(json!([{"nodes": [deep]}]))),
        Some(Limit::Depth)
    );
    let huge = json!({"nodes": [{"i": u64::from(u32::MAX) + 1, "keep": ""}]});
    assert_eq!(refused(translate(json!([huge]))), Some(Limit::Id));
    let duplicate = json!({"nodes": [{"i": 0, "nodes": []}, {"i": 0, "keep": ""}]});
    assert_eq!(refused(translate(json!([duplicate]))), None);
}

#[test]
fn detection_prefers_content_over_declaration() {
    const ENGLISH: &str = "The quick brown fox jumps over the lazy dog. This page explains how to install the package and configure the service so that it starts automatically when the computer boots.";
    const JAPANESE: &str = "このページでは、パッケージをインストールして、コンピューターの起動時にサービスが自動的に開始されるように設定する方法を説明します。";
    for (lang, sample, expected) in [
        (Some("en"), JAPANESE, Some("ja")),
        (Some("ja"), ENGLISH, Some("en")),
        (None, ENGLISH, Some("en")),
        (Some("de-DE"), "", Some("de")),
        (Some("fr"), "OK", Some("fr")),
        (None, "", None),
        (Some("klingon"), "", None),
    ] {
        let report = PageReport {
            lang: lang.map(str::to_owned),
            sample: sample.to_owned(),
        };
        assert_eq!(
            detect(&report).map(|language| language.tag()),
            expected,
            "{lang:?} {sample}"
        );
    }
}
