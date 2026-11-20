use rikka_compositor_desktop::{Drawn, Frame, Hit, SceneLayers, Target, WindowId};
use rikka_compositor_shell::PopupParent;

use crate::common::{World, point, rect};

const FRAME: Frame = Frame {
    title: 40,
    reach: 8,
};

fn framed(world: &mut World) -> WindowId {
    let window = world.window(rect(100, 100, 200, 150));
    world
        .desktop
        .window_mut(window)
        .expect("the window")
        .set_frame(FRAME);
    window
}

fn hit(world: &World, x: f64, y: f64) -> Option<Hit> {
    world
        .desktop
        .hit_under(&world.trees, point(x, y), |_| SceneLayers::ALL)
}

#[test]
fn frame_keeps_content_below_the_title() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let window = framed(&mut world);
    let record = world.desktop.window(window).expect("the window");

    assert_eq!(record.geometry(), rect(100, 100, 200, 190));
    assert_eq!(record.content(), rect(100, 140, 200, 150));
    assert!(
        world
            .drawn(output, SceneLayers::ALL)
            .iter()
            .any(|drawn| matches!(
                drawn,
                Drawn::Surface { target: Target::Window(drawn), location, .. }
                    if *drawn == window && *location == rect(100, 140, 0, 0).loc
            ))
    );

    world
        .desktop
        .window_mut(window)
        .expect("the window")
        .set_frame(Frame::NONE);
    assert_eq!(
        world.desktop.window(window).expect("the window").geometry(),
        rect(100, 100, 200, 150)
    );
}

#[test]
fn frame_answers_hits_its_surface_misses() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let lower = world.window(rect(0, 0, 800, 800));
    let window = framed(&mut world);

    let cases = [
        ("title", (150.0, 120.0), Some(Target::Window(window)), true),
        ("reach", (95.0, 200.0), Some(Target::Window(window)), true),
        (
            "content",
            (150.0, 200.0),
            Some(Target::Window(window)),
            false,
        ),
        (
            "beyond reach",
            (90.0, 200.0),
            Some(Target::Window(lower)),
            false,
        ),
    ];
    for (name, (x, y), target, on_frame) in cases {
        let found = hit(&world, x, y);
        assert_eq!(found.map(|hit| hit.target()), target, "{name}");
        assert_eq!(matches!(found, Some(Hit::Frame { .. })), on_frame, "{name}");
    }
    assert_eq!(
        hit(&world, 150.0, 120.0),
        Some(Hit::Frame {
            window,
            local: point(50.0, 20.0)
        })
    );
    assert_eq!(
        world
            .desktop
            .element_under(&world.trees, point(150.0, 120.0), |_| SceneLayers::ALL),
        None,
        "a frame shields the window below"
    );
}

#[test]
fn popups_open_from_the_content() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let window = framed(&mut world);
    let menu = world.popup(
        PopupParent::Toplevel(world.toplevel_of(window)),
        rect(10, 20, 50, 50),
    );

    assert_eq!(
        world.desktop.popup_location(menu),
        Some(rect(110, 160, 0, 0).loc)
    );
}
