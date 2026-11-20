use animate_track::{Edge, Span};

const SNAP: i64 = 3;

fn near(playhead: i64) -> impl Fn(i64) -> Option<i64> {
    move |frame| {
        let nudge = playhead - frame;
        (nudge.abs() <= SNAP).then_some(nudge)
    }
}

#[test]
fn dragged_spans_keep_their_length_and_snap_to_the_playhead() {
    let closed = Span {
        start: 10,
        end: Some(30),
    };
    let open = Span {
        start: 10,
        end: None,
    };
    let cases = [
        (closed, Edge::Body, 15.0, 100, (25, Some(45))),
        (closed, Edge::Body, 14.0, 46, (26, Some(46))),
        (closed, Edge::Body, -40.0, 100, (0, Some(0))),
        (closed, Edge::Start, 8.0, 20, (20, Some(30))),
        (closed, Edge::End, -5.0, 100, (10, Some(25))),
        (open, Edge::Body, 4.0, 100, (14, None)),
        (open, Edge::End, 42.0, 100, (10, Some(42))),
    ];
    for (span, edge, by, playhead, (start, end)) in cases {
        let dragged = span.dragged(edge, 0.0, by, near(playhead));
        assert_eq!(
            (dragged.start, dragged.end),
            (start, end),
            "{span:?} {edge:?} by {by}"
        );
    }
}
