use rikka_compositor_planes::{Presentation, VrrPolicy, presentation};
use rikka_compositor_types::{ContentType, Flip, Pacing};

struct Case {
    name: &'static str,
    policy: VrrPolicy,
    fullscreen: Option<ContentType>,
    flip: Flip,
    vrr: bool,
    pacing: Pacing,
}

const fn case(
    name: &'static str,
    policy: VrrPolicy,
    fullscreen: Option<ContentType>,
    flip: Flip,
    vrr: bool,
    pacing: Pacing,
) -> Case {
    Case {
        name,
        policy,
        fullscreen,
        flip,
        vrr,
        pacing,
    }
}

#[test]
fn presentation_follows_content_and_policy() {
    let cases = [
        case(
            "the desktop keeps a fixed refresh rate",
            VrrPolicy::Automatic,
            None,
            Flip::Vsync,
            false,
            Pacing::Balanced,
        ),
        case(
            "a fullscreen game gets variable refresh and low-latency pacing",
            VrrPolicy::Automatic,
            Some(ContentType::Game),
            Flip::Vsync,
            true,
            Pacing::LowLatency,
        ),
        case(
            "a fullscreen video gets variable refresh and steady pacing",
            VrrPolicy::Automatic,
            Some(ContentType::Video),
            Flip::Vsync,
            true,
            Pacing::Steady,
        ),
        case(
            "a fullscreen photo keeps a fixed refresh rate",
            VrrPolicy::Automatic,
            Some(ContentType::Photo),
            Flip::Vsync,
            false,
            Pacing::Balanced,
        ),
        case(
            "a fullscreen surface without a content type gets variable refresh",
            VrrPolicy::Automatic,
            Some(ContentType::None),
            Flip::Vsync,
            true,
            Pacing::Balanced,
        ),
        case(
            "a tearing surface is paced by its own commits",
            VrrPolicy::Automatic,
            Some(ContentType::Game),
            Flip::Async,
            true,
            Pacing::Immediate,
        ),
        case(
            "variable refresh never engages when the display forbids it",
            VrrPolicy::Never,
            Some(ContentType::Game),
            Flip::Vsync,
            false,
            Pacing::LowLatency,
        ),
        case(
            "variable refresh stays on for the desktop when the display always wants it",
            VrrPolicy::Always,
            None,
            Flip::Vsync,
            true,
            Pacing::Balanced,
        ),
    ];
    for case in cases {
        assert_eq!(
            presentation(case.policy, case.fullscreen, case.flip),
            Presentation {
                vrr: case.vrr,
                pacing: case.pacing,
            },
            "{}",
            case.name
        );
    }
}
