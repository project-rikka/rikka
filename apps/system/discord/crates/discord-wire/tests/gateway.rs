use discord_wire::{Closing, Frame, Hello};

#[test]
fn close_codes_choose_how_to_reconnect() {
    let cases = [
        (None, Closing::Resume),
        (Some(1006), Closing::Resume),
        (Some(4000), Closing::Resume),
        (Some(1000), Closing::Restart),
        (Some(1001), Closing::Restart),
        (Some(4007), Closing::Restart),
        (Some(4009), Closing::Restart),
        (Some(4004), Closing::SignedOut),
        (Some(4010), Closing::SignedOut),
        (Some(4014), Closing::SignedOut),
    ];
    for (code, expected) in cases {
        assert_eq!(Closing::from_code(code), expected, "{code:?}");
    }
}

#[test]
fn frames_keep_dispatch_data_raw() {
    let text = r#"{"op":0,"s":12,"t":"MESSAGE_CREATE","d":{"id":"1","content":"hi"}}"#;
    let frame: Frame<'_> = serde_json::from_str(text).unwrap();
    assert_eq!(frame.op, 0);
    assert_eq!(frame.s, Some(12));
    assert_eq!(frame.t, Some("MESSAGE_CREATE"));
    assert_eq!(frame.d.unwrap().get(), r#"{"id":"1","content":"hi"}"#);
    let hello: Frame<'_> =
        serde_json::from_str(r#"{"op":10,"s":null,"t":null,"d":{"heartbeat_interval":41250}}"#)
            .unwrap();
    let interval: Hello = serde_json::from_str(hello.d.unwrap().get()).unwrap();
    assert_eq!(interval.heartbeat_interval, 41250);
}
