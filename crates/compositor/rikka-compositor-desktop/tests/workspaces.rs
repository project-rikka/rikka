use crate::common::{World, rect};
use rikka_compositor_desktop::{Visibility, Workspace};

#[test]
fn switching_workspace_swaps_visible_windows() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let first = world.window(rect(10, 10, 100, 100));
    let waiting = world.window_on(rect(10, 10, 100, 100), Workspace::new(output, 2));
    assert_eq!(
        world
            .desktop
            .window(waiting)
            .map(|window| window.visibility()),
        Some(Visibility::OnHiddenWorkspace)
    );

    let switch = world
        .desktop
        .show_workspace(Workspace::new(output, 2))
        .expect("a known workspace")
        .expect("a switch");
    assert_eq!(switch.from, Workspace::new(output, 1));
    assert_eq!(switch.leaving, [first]);
    assert_eq!(switch.arriving, [waiting]);
    assert_eq!(
        world
            .desktop
            .window(first)
            .map(|window| window.visibility()),
        Some(Visibility::OnHiddenWorkspace)
    );
    assert_eq!(
        world
            .desktop
            .window(waiting)
            .map(|window| window.visibility()),
        Some(Visibility::Shown)
    );
    assert!(
        world
            .desktop
            .window_for_toplevel(world.toplevel_of(first))
            .is_some()
    );

    assert_eq!(
        world.desktop.show_workspace(Workspace::new(output, 2)),
        Ok(None)
    );
}

#[test]
fn removed_output_moves_windows_to_same_workspace() {
    let mut world = World::new(10);
    let staying = world.output(rect(0, 0, 1000, 800));
    let going = world.output(rect(1000, 0, 1000, 800));
    let shown = world.window(rect(1100, 100, 200, 200));
    let parked = world.window_on(rect(1100, 100, 200, 200), Workspace::new(going, 7));
    let untouched = world.window(rect(100, 100, 200, 200));

    let removed = world.desktop.remove_output(going).expect("a known display");
    assert_eq!(removed.windows, [shown, parked]);
    let workspace_of = |id| {
        world
            .desktop
            .window(id)
            .and_then(|window| window.workspace())
    };
    assert_eq!(workspace_of(shown), Some(Workspace::new(staying, 1)));
    assert_eq!(workspace_of(parked), Some(Workspace::new(staying, 7)));
    assert_eq!(workspace_of(untouched), Some(Workspace::new(staying, 1)));
    assert_eq!(
        world
            .desktop
            .window(parked)
            .map(|window| window.visibility()),
        Some(Visibility::OnHiddenWorkspace)
    );
}

#[test]
fn orphan_windows_move_to_next_output() {
    let mut world = World::new(10);
    let only = world.output(rect(0, 0, 1000, 800));
    let window = world.window_on(rect(10, 10, 100, 100), Workspace::new(only, 3));
    world.desktop.remove_output(only).expect("a known display");
    assert_eq!(
        world
            .desktop
            .window(window)
            .and_then(|window| window.workspace()),
        None
    );

    let next = world.output(rect(0, 0, 1280, 720));
    assert_eq!(
        world
            .desktop
            .window(window)
            .and_then(|window| window.workspace()),
        Some(Workspace::new(next, 1))
    );
    assert_eq!(
        world
            .desktop
            .window(window)
            .map(|window| window.visibility()),
        Some(Visibility::Shown)
    );
}
