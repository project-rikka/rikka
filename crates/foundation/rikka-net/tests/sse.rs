use std::time::Duration;

use rikka_net::EventDecoder;

#[test]
fn decodes_standard_event_stream_across_chunks() {
    let stream = "\u{feff}id: a\0b\ndata: x\n\n: keep-alive\r\n\r\nevent: ping\n\ndata: cr\r\rdata: one\r\ndata: two\r\n\r\nid: 7\ndata\n\ndata: b\n\n";
    let mut decoder = EventDecoder::new();
    let mut events = Vec::new();
    for chunk in stream.as_bytes().chunks(3) {
        decoder.push(chunk);
        while let Some(event) = decoder.next_event() {
            events.push((
                event.event().to_owned(),
                event.data().to_owned(),
                event.id().map(str::to_owned),
            ));
        }
    }

    let message = |data: &str, id: Option<&str>| {
        ("message".to_owned(), data.to_owned(), id.map(str::to_owned))
    };
    assert_eq!(
        events,
        [
            message("x", None),
            message("cr", None),
            message("one\ntwo", None),
            message("", Some("7")),
            message("b", Some("7")),
        ]
    );
}

#[test]
fn commits_last_event_id_and_reconnection_time() {
    let mut decoder = EventDecoder::new();

    decoder.push(b"retry: 250\nretry: soon\nid: 4\n");
    assert_eq!(decoder.last_event_id(), None);

    decoder.push(b"\n");
    assert_eq!(decoder.last_event_id(), Some("4"));
    assert_eq!(
        decoder.reconnection_time(),
        Some(Duration::from_millis(250))
    );
    assert!(decoder.next_event().is_none());
}
