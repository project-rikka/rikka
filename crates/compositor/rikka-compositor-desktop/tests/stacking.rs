use crate::common::{World, rect};
use rikka_compositor_desktop::{Drawn, SceneLayers};
use rikka_compositor_desktop::{Error, Stack};

#[test]
fn windows_stack_by_arrival_and_restack() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let first = world.window(rect(10, 10, 100, 100));
    let second = world.window(rect(20, 20, 100, 100));
    let third = world.window(rect(30, 30, 100, 100));
    assert_eq!(
        world.desktop.stacking().collect::<Vec<_>>(),
        [first, second, third]
    );

    assert_eq!(world.desktop.raise(first), Ok(true));
    assert_eq!(
        world.desktop.stacking().collect::<Vec<_>>(),
        [second, third, first]
    );
    assert_eq!(world.desktop.raise(first), Ok(false));

    assert_eq!(world.desktop.lower(third), Ok(true));
    assert_eq!(
        world.desktop.stacking().collect::<Vec<_>>(),
        [third, second, first]
    );
    assert_eq!(world.desktop.lower(third), Ok(false));
    assert_eq!(world.desktop.restack(third, Stack::Above(third)), Ok(false));
}

#[test]
fn override_redirect_stacks_above_managed() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let menu = world.unmanaged_window(rect(10, 10, 100, 100));
    let managed = world.window(rect(20, 20, 100, 100));
    world.desktop.raise(managed).expect("raised");

    assert_eq!(
        world.desktop.stacking().collect::<Vec<_>>(),
        [managed, menu]
    );
    assert_eq!(
        world.desktop.restack(managed, Stack::Above(menu)),
        Err(Error::SeparateStacks {
            window: managed,
            other: menu
        })
    );
}

fn drawn_windows(world: &World, output: rikka_compositor_types::OutputId) -> Vec<Drawn> {
    world.drawn(output, SceneLayers::ALL)
}

#[test]
fn minimized_window_is_hidden() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let window = world.window(rect(100, 100, 400, 300));
    let toplevel = world.toplevel_of(window);

    assert_eq!(world.desktop.minimize(window), Ok(true));
    assert_eq!(world.desktop.minimize(window), Ok(false));
    assert_eq!(world.desktop.window_for_toplevel(toplevel), Some(window));
    assert!(drawn_windows(&world, output).is_empty());
    assert_eq!(
        world
            .desktop
            .element_under(&world.trees, crate::common::point(200.0, 200.0), |_| {
                SceneLayers::ALL
            }),
        None
    );
}

#[test]
fn topmost_fullscreen_window_owns_output() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let player = world.window(rect(0, 0, 1920, 1080));
    world
        .desktop
        .window_mut(player)
        .expect("the window")
        .set_placement(rikka_compositor_desktop::Placement::Fullscreen);
    assert_eq!(world.desktop.fullscreen_on(output), Some(player));

    let dialog = world.window(rect(100, 100, 300, 200));
    assert_eq!(world.desktop.fullscreen_on(output), None);
    world.desktop.minimize(dialog).expect("minimized");
    assert_eq!(world.desktop.fullscreen_on(output), Some(player));
}
