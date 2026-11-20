use rikka_remote_wire::{Button, FileHeader, InputEvent, Parsed, Position};

#[test]
fn input_events_parse_back_from_a_stream() {
    let events = [
        InputEvent::Key {
            code: 125,
            pressed: true,
        },
        InputEvent::Button {
            button: Button::Secondary,
            pressed: false,
            at: Position { x: 100, y: 65535 },
        },
        InputEvent::Wheel { dx: -120, dy: 360 },
        InputEvent::Scroll {
            dx: 0.5,
            dy: -12.25,
        },
        InputEvent::Relative { dx: 3.0, dy: -1.5 },
        InputEvent::Text("日本語".into()),
        InputEvent::ReleaseAll,
    ];
    let mut stream = Vec::new();
    for event in &events {
        event.encode(&mut stream);
    }
    let mut parsed = Vec::new();
    let mut rest = &stream[..];
    while !rest.is_empty() {
        match InputEvent::parse(rest).unwrap() {
            Parsed::Complete { value, consumed } => {
                parsed.push(value);
                rest = &rest[consumed..];
            }
            Parsed::Incomplete => panic!("a whole stream parsed as incomplete"),
        }
    }
    assert_eq!(parsed, events);
}

#[test]
fn partial_events_wait_for_more_bytes() {
    let mut bytes = Vec::new();
    InputEvent::Text("hello".into()).encode(&mut bytes);
    for cut in 0..bytes.len() {
        assert_eq!(InputEvent::parse(&bytes[..cut]), Ok(Parsed::Incomplete));
    }
    assert!(InputEvent::parse(&[0x7F]).is_err());
}

#[test]
fn file_headers_parse_incrementally() {
    let header = FileHeader {
        id: 4,
        name: "report.pdf".into(),
        size: 1 << 33,
    };
    let mut bytes = Vec::new();
    header.encode(&mut bytes);
    bytes.extend_from_slice(b"contents");
    assert_eq!(FileHeader::parse(&bytes[..6]), Ok(Parsed::Incomplete));
    assert_eq!(
        FileHeader::parse(&bytes),
        Ok(Parsed::Complete {
            value: header,
            consumed: bytes.len() - 8
        })
    );
}
