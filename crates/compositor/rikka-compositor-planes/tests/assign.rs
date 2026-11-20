use rikka_compositor_geometry::{Buffer, Point, Rect, Size, Transform};
use rikka_compositor_planes::{
    Assignment, Blocked, Display, Layout, Reason, Surface, Tearing, assign,
};
use rikka_compositor_types::{Flip, Format, FormatSet, Fourcc, Modifier, TearingHint};

const OPAQUE: Format = Format::new(Fourcc::XRGB8888, Modifier::LINEAR);
const FOREIGN: Format = Format::new(Fourcc(0x3231_564e), Modifier::LINEAR);

fn rect<S>(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, S> {
    Rect::new(Point::new(x, y), Size::new(w, h))
}

fn fullscreen(id: u32) -> Surface<u32> {
    Surface {
        id,
        format: OPAQUE,
        buffer: Size::new(1920, 1080),
        source: Rect::<i32, Buffer>::from_size(Size::new(1920, 1080)).to_f64(),
        transform: Transform::Normal,
        placement: rect(0, 0, 1920, 1080),
        opaque: true,
        covered: false,
        tearing: TearingHint::Vsync,
    }
}

pub(crate) fn scanned_out(surface: u32) -> Assignment<u32> {
    flipped(surface, Flip::Vsync)
}

fn flipped(surface: u32, flip: Flip) -> Assignment<u32> {
    Assignment::Primary(Layout {
        surface,
        format: OPAQUE,
        flip,
    })
}

fn blocked(surface: u32, reason: Reason) -> Assignment<u32> {
    Assignment::Composited(Some(Blocked { surface, reason }))
}

struct Case {
    name: &'static str,
    transform: Transform,
    tearing: Tearing,
    surface: Option<Surface<u32>>,
    refused: Option<Format>,
    expected: Assignment<u32>,
}

impl Case {
    fn new(name: &'static str, surface: Option<Surface<u32>>, expected: Assignment<u32>) -> Self {
        Self {
            name,
            transform: Transform::Normal,
            tearing: Tearing::Forbidden,
            surface,
            refused: None,
            expected,
        }
    }
}

#[test]
fn assigns_fullscreen_scanout_or_reason() {
    let primary: FormatSet = [OPAQUE].into_iter().collect();
    let cases = vec![
        Case::new(
            "an opaque fullscreen buffer the size of the mode takes the primary plane",
            Some(fullscreen(1)),
            scanned_out(1),
        ),
        Case::new(
            "without a surface covering the display everything is composited",
            None,
            Assignment::Composited(None),
        ),
        Case::new(
            "a fullscreen surface without opaque content is composited",
            Some(Surface {
                opaque: false,
                ..fullscreen(1)
            }),
            blocked(1, Reason::Translucent),
        ),
        Case::new(
            "a fullscreen buffer smaller than the mode is composited",
            Some(Surface {
                buffer: Size::new(1280, 720),
                source: Rect::<i32, Buffer>::from_size(Size::new(1280, 720)).to_f64(),
                ..fullscreen(1)
            }),
            blocked(1, Reason::Size),
        ),
        Case::new(
            "anything drawn over a fullscreen window keeps it off the primary plane",
            Some(Surface {
                covered: true,
                ..fullscreen(1)
            }),
            blocked(1, Reason::Covered),
        ),
        Case::new(
            "a fullscreen format the primary plane lacks asks for the primary plane's formats",
            Some(Surface {
                format: FOREIGN,
                ..fullscreen(1)
            }),
            blocked(1, Reason::Format),
        ),
        Case::new(
            "a crop inside the buffer is composited",
            Some(Surface {
                source: Rect::new(Point::new(0.0, 16.0), Size::new(1920.0, 1064.0)),
                ..fullscreen(1)
            }),
            blocked(1, Reason::Source),
        ),
        Case::new(
            "a surface placed off the display's origin is composited",
            Some(Surface {
                placement: rect(-10, 0, 1940, 1080),
                ..fullscreen(1)
            }),
            blocked(1, Reason::Placement),
        ),
        Case {
            refused: Some(OPAQUE),
            ..Case::new(
                "a format the display refused before is not tried again",
                Some(fullscreen(1)),
                blocked(1, Reason::Refused),
            )
        },
        Case {
            transform: Transform::Rotate90,
            ..Case::new(
                "a buffer already turned like the display takes the primary plane",
                Some(Surface {
                    transform: Transform::Rotate90,
                    placement: rect(0, 0, 1080, 1920),
                    ..fullscreen(1)
                }),
                scanned_out(1),
            )
        },
        Case {
            transform: Transform::Rotate90,
            ..Case::new(
                "a buffer turned unlike the display is composited",
                Some(Surface {
                    placement: rect(0, 0, 1080, 1920),
                    ..fullscreen(1)
                }),
                blocked(1, Reason::Transform),
            )
        },
        Case {
            tearing: Tearing::Allowed,
            ..Case::new(
                "a surface asking for async presentation flips asynchronously where tearing is allowed",
                Some(Surface {
                    tearing: TearingHint::Async,
                    ..fullscreen(1)
                }),
                flipped(1, Flip::Async),
            )
        },
        Case::new(
            "a surface asking for async presentation stays vsynced where tearing is forbidden",
            Some(Surface {
                tearing: TearingHint::Async,
                ..fullscreen(1)
            }),
            scanned_out(1),
        ),
        Case {
            tearing: Tearing::Allowed,
            ..Case::new(
                "a surface asking for vsync stays vsynced where tearing is allowed",
                Some(fullscreen(1)),
                scanned_out(1),
            )
        },
    ];

    for case in cases {
        let display = Display::new(Size::new(1920, 1080), case.transform, &primary)
            .with_tearing(case.tearing);
        let assignment = assign(&display, case.surface.as_ref(), |format| {
            case.refused == Some(format)
        });
        assert_eq!(assignment, case.expected, "{}", case.name);
    }
}
