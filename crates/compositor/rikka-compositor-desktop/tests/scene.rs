use crate::common::{World, layer_state, point, rect};
use rikka_compositor_desktop::{Drawn, LayerOrder, SceneLayers, SurfaceTrees, Target, Workspace};
use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::Anchor;
use rikka_compositor_shell::{PopupParent, ShellRole};

fn targets(drawn: &[Drawn]) -> Vec<Target> {
    drawn
        .iter()
        .filter_map(|drawn| match drawn {
            Drawn::Surface { target, .. } => Some(*target),
            Drawn::Snapshot { .. } => None,
        })
        .collect()
}

fn under(world: &World, x: f64, y: f64, around: SceneLayers) -> Option<Target> {
    world
        .desktop
        .element_under(&world.trees, point(x, y), |_| around)
        .map(|under| under.target)
}

#[test]
fn output_draws_layers_in_stacking_order() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let overlay = world.layer(output, layer_state(Level::Overlay, Anchor::all(), 0, 0, 0));
    let top = world.layer(output, layer_state(Level::Top, Anchor::TOP, 0, 100, 30));
    let bottom = world.layer(
        output,
        layer_state(Level::Bottom, Anchor::BOTTOM, 0, 100, 30),
    );
    let wallpaper = world.layer(
        output,
        layer_state(Level::Background, Anchor::all(), -1, 0, 0),
    );
    world.arrange(output);
    let lower = world.window(rect(100, 100, 400, 300));
    let upper = world.window(rect(200, 200, 400, 300));
    let menu = world.popup(
        PopupParent::Toplevel(world.toplevel_of(lower)),
        rect(10, 20, 80, 90),
    );
    let hidden = world.window_on(rect(0, 0, 100, 100), Workspace::new(output, 2));

    let drawn = world.drawn(output, SceneLayers::ALL);
    assert_eq!(
        targets(&drawn),
        [
            Target::Layer(wallpaper),
            Target::Layer(bottom),
            Target::Window(lower),
            Target::Popup(menu),
            Target::Window(upper),
            Target::Layer(top),
            Target::Layer(overlay),
        ]
    );
    assert!(!targets(&drawn).contains(&Target::Window(hidden)));
    let location_of = |target_to_find| {
        drawn.iter().find_map(|drawn| match drawn {
            Drawn::Surface {
                target, location, ..
            } if *target == target_to_find => Some(*location),
            _ => None,
        })
    };
    assert_eq!(location_of(Target::Popup(menu)), Some(Point::new(110, 120)));
    assert_eq!(location_of(Target::Layer(top)), Some(Point::new(910, 0)));

    let over_the_desktop = world.drawn(output, SceneLayers::WITHOUT_WINDOWS);
    assert!(!targets(&over_the_desktop).contains(&Target::Window(lower)));
    assert!(!targets(&over_the_desktop).contains(&Target::Popup(menu)));
}

#[test]
fn topmost_layer_stays_above_later_layers_of_its_level() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let state = layer_state(Level::Overlay, Anchor::all(), 0, 0, 0);
    let earlier = world.layer(output, state);
    let panel = world.ordered_layer(output, state, LayerOrder::Topmost);
    let later = world.layer(output, state);
    let top = world.layer(output, layer_state(Level::Top, Anchor::all(), 0, 0, 0));
    world.arrange(output);

    assert_eq!(
        targets(&world.drawn(output, SceneLayers::ALL)),
        [
            Target::Layer(top),
            Target::Layer(earlier),
            Target::Layer(later),
            Target::Layer(panel),
        ]
    );
}

#[test]
fn captured_window_draws_over_wallpapers_only() {
    let mut world = World::new(10);
    let left = world.output(rect(0, 0, 800, 600));
    let right = world.output(rect(800, 0, 800, 600));
    let wallpapers = [left, right].map(|output| {
        let wallpaper = world.layer(
            output,
            layer_state(Level::Background, Anchor::all(), -1, 0, 0),
        );
        world.popup(PopupParent::Layer(wallpaper), rect(40, 30, 120, 90));
        for level in [Level::Bottom, Level::Top, Level::Overlay] {
            world.layer(output, layer_state(level, Anchor::all(), -1, 0, 0));
        }
        world.arrange(output);
        wallpaper
    });
    world.window(rect(500, 100, 600, 400));
    let captured = world.window(rect(600, 200, 400, 300));
    let menu = world.popup(
        PopupParent::Toplevel(world.toplevel_of(captured)),
        rect(10, 20, 80, 90),
    );
    world.window(rect(650, 250, 200, 100));

    let mut drawn = Vec::new();
    world
        .desktop
        .drawn_window_on_wallpapers_into(captured, &mut drawn);
    assert_eq!(
        targets(&drawn),
        [
            Target::Layer(wallpapers[0]),
            Target::Layer(wallpapers[1]),
            Target::Window(captured),
            Target::Popup(menu),
        ]
    );
}

#[test]
fn confined_window_stays_on_its_output() {
    let mut world = World::new(10);
    let own = world.output(rect(0, 0, 1000, 800));
    let beside = world.output(rect(1000, 0, 1000, 800));
    let window = world.window_on(rect(700, 100, 600, 300), Workspace::new(own, 1));
    let probe = |world: &World| {
        (
            targets(&world.drawn(own, SceneLayers::ALL)).contains(&Target::Window(window)),
            targets(&world.drawn(beside, SceneLayers::ALL)).contains(&Target::Window(window)),
            under(world, 1100.0, 200.0, SceneLayers::ALL),
        )
    };
    assert_eq!(probe(&world), (true, true, Some(Target::Window(window))));

    world
        .desktop
        .window_mut(window)
        .expect("a known window")
        .set_confined(true);
    assert_eq!(probe(&world), (true, false, None));
    assert_eq!(
        under(&world, 900.0, 200.0, SceneLayers::ALL),
        Some(Target::Window(window))
    );
}

#[test]
fn input_region_passes_pointer_through() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let lower = world.window(rect(0, 0, 400, 400));
    let upper = world.window(rect(0, 0, 400, 400));
    let surface = world
        .desktop
        .window(upper)
        .and_then(|window| window.surface())
        .expect("a surface");
    world.trees.set_input(surface, rect(0, 0, 100, 100));

    let found = world
        .desktop
        .element_under(&world.trees, point(50.5, 60.25), |_| SceneLayers::ALL)
        .expect("something");
    assert_eq!(found.target, Target::Window(upper));
    assert_eq!(found.surface, surface);
    assert_eq!(found.local, point(50.5, 60.25));
    assert_eq!(
        under(&world, 200.0, 200.0, SceneLayers::ALL),
        Some(Target::Window(lower))
    );
}

#[test]
fn locked_desktop_shows_only_lock_surfaces() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    world.window(rect(0, 0, 400, 400));
    world.layer(output, layer_state(Level::Overlay, Anchor::all(), 0, 0, 0));
    world.arrange(output);

    world.desktop.lock();
    assert!(world.drawn(output, SceneLayers::ALL).is_empty());
    assert_eq!(under(&world, 10.0, 10.0, SceneLayers::ALL), None);

    let (lock, surface) = world.lock_surface(output);
    assert_eq!(
        world.drawn(output, SceneLayers::ALL),
        [Drawn::Surface {
            target: Target::Lock(lock),
            root: surface,
            location: Point::new(0, 0)
        }]
    );
    assert_eq!(
        under(&world, 10.0, 10.0, SceneLayers::ALL),
        Some(Target::Lock(lock))
    );
    assert_eq!(
        world.desktop.target_for_role(ShellRole::LockSurface(lock)),
        Some(Target::Lock(lock))
    );

    world.desktop.unlock();
    assert!(world.desktop.lock_surface_on(output).is_none());
    assert_eq!(world.drawn(output, SceneLayers::ALL).len(), 2);
}

fn drawn_hit(world: &World, x: f64, y: f64, around: SceneLayers) -> Option<Target> {
    let at = point(x, y);
    let output = world.desktop.output_at(at)?;
    world
        .drawn(output, around)
        .into_iter()
        .rev()
        .find_map(|drawn| {
            let Drawn::Surface {
                target,
                root,
                location,
            } = drawn
            else {
                return None;
            };
            world
                .trees
                .surface_under(root, at - location.to_f64())
                .map(|_| target)
        })
}

#[test]
fn hit_test_follows_draw_order() {
    let mut world = World::new(10);
    let left = world.output(rect(0, 0, 800, 600));
    let right = world.output(rect(800, 0, 800, 600));
    for (level, anchor) in [
        (Level::Background, Anchor::all()),
        (Level::Bottom, Anchor::BOTTOM),
        (Level::Top, Anchor::TOP),
        (Level::Overlay, Anchor::RIGHT),
    ] {
        for output in [left, right] {
            let layer = world.layer(output, layer_state(level, anchor, 0, 300, 200));
            world.arrange(output);
            let menu = world.popup(PopupParent::Layer(layer), rect(40, 30, 120, 90));
            world.popup(PopupParent::Popup(menu), rect(60, 50, 90, 70));
        }
    }
    let lower = world.window(rect(50, 50, 400, 300));
    let upper = world.window(rect(600, 100, 400, 300));
    let lower_menu = world.popup(
        PopupParent::Toplevel(world.toplevel_of(lower)),
        rect(300, 200, 200, 150),
    );
    world.popup(PopupParent::Popup(lower_menu), rect(150, 100, 100, 100));
    world.popup(PopupParent::Popup(lower_menu), rect(-50, -20, 100, 100));
    world.popup(
        PopupParent::Toplevel(world.toplevel_of(upper)),
        rect(100, 250, 150, 150),
    );
    world.unmanaged_window(rect(700, 250, 200, 80));
    let minimized = world.window(rect(0, 0, 800, 600));
    world.desktop.minimize(minimized).expect("minimized");

    let everything = [
        SceneLayers::ALL,
        SceneLayers::WITHOUT_WINDOWS,
        SceneLayers {
            top: false,
            windows: true,
            backdrop: false,
        },
    ];
    for around in everything {
        for y in (0..600).step_by(7) {
            for x in (0..1600).step_by(11) {
                let (x, y) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert_eq!(
                    under(&world, x, y, around),
                    drawn_hit(&world, x, y, around),
                    "at {x},{y} with {around:?}"
                );
            }
        }
    }
}

#[test]
fn deep_popup_chain_draws_iteratively() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let window = world.window(rect(0, 0, 400, 400));
    let mut parent = PopupParent::Toplevel(world.toplevel_of(window));
    let mut deepest = None;
    for _ in 0..20_000 {
        let popup = world.popup(parent, rect(0, 0, 10, 10));
        parent = PopupParent::Popup(popup);
        deepest = Some(popup);
    }
    let deepest = deepest.expect("a popup");
    assert_eq!(world.drawn(output, SceneLayers::ALL).len(), 20_001);
    assert_eq!(
        under(&world, 5.0, 5.0, SceneLayers::ALL),
        Some(Target::Popup(deepest))
    );
}
