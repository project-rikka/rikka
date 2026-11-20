use crate::common::{World, layer_state, rect};
use rikka_compositor_desktop::LayerConfigure;
use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
use rikka_compositor_shell::Margins;

fn across_top() -> Anchor {
    Anchor::TOP | Anchor::LEFT | Anchor::RIGHT
}

#[test]
fn exclusive_zone_shrinks_work_area() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let panel = world.layer(output, layer_state(Level::Top, across_top(), 40, 0, 40));

    let arrangement = world.arrange(output);
    assert_eq!(
        arrangement.configures,
        [LayerConfigure {
            layer: panel,
            size: Size::new(1920, 40)
        }]
    );
    assert!(arrangement.work_area_changed);
    assert!(arrangement.closed.is_empty());
    assert_eq!(
        world.desktop.layer(panel).map(|layer| layer.geometry()),
        Some(rect(0, 0, 1920, 40))
    );
    assert_eq!(
        world
            .desktop
            .output(output)
            .map(|output| output.work_area()),
        Some(rect(0, 40, 1920, 1040))
    );
}

#[test]
fn arrangement_follows_the_published_layout() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let panel = world.layer(output, layer_state(Level::Top, across_top(), 40, 0, 40));
    world.arrange(output);

    world.place_output(output, rect(1920, 0, 1280, 720));
    let arrangement = world.arrange(output);
    assert_eq!(
        arrangement.configures,
        [LayerConfigure {
            layer: panel,
            size: Size::new(1280, 40)
        }]
    );
    assert_eq!(
        world.desktop.layer(panel).map(|layer| layer.geometry()),
        Some(rect(1920, 0, 1280, 40))
    );
    assert_eq!(
        world
            .desktop
            .output(output)
            .map(|output| output.work_area()),
        Some(rect(1920, 40, 1280, 680))
    );
    assert_eq!(
        world.desktop.output_at(crate::common::point(2000.0, 10.0)),
        Some(output)
    );
}

#[test]
fn closes_surface_exceeding_margins() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let mut squeezed = layer_state(Level::Top, across_top(), 0, 0, 40);
    squeezed.margins = Margins {
        left: 1000,
        right: 1000,
        ..Margins::default()
    };
    let squeezed = world.layer(output, squeezed);

    let arrangement = world.arrange(output);
    assert_eq!(arrangement.closed, [squeezed]);
    assert!(arrangement.configures.is_empty());
    assert!(world.desktop.layer(squeezed).is_none());
}

#[test]
fn topmost_exclusive_layer_takes_keyboard() {
    let mut world = World::new(10);
    let output = world.output(rect(0, 0, 1920, 1080));
    let exclusive = |level| {
        let mut state = layer_state(level, Anchor::all(), 0, 0, 0);
        state.keyboard_interactivity = KeyboardInteractivity::Exclusive;
        state
    };
    let below = world.layer(output, exclusive(Level::Bottom));
    let launcher = world.layer(output, exclusive(Level::Top));
    let unmapped = world.layer(output, exclusive(Level::Overlay));
    for layer in [below, launcher] {
        world.desktop.set_layer_mapped(layer, true).expect("mapped");
    }
    assert_eq!(world.desktop.keyboard_layer(output), Some(launcher));

    world
        .desktop
        .set_layer_mapped(unmapped, true)
        .expect("mapped");
    assert_eq!(world.desktop.keyboard_layer(output), Some(unmapped));

    world.desktop.remove_layer(unmapped);
    world.desktop.remove_layer(launcher);
    assert_eq!(world.desktop.keyboard_layer(output), None);
}

#[test]
fn extreme_margins_and_zones_arrange_without_overflow() {
    let extremes = [i32::MIN, i32::MAX];
    for (margin, zone) in extremes
        .into_iter()
        .flat_map(|margin| extremes.map(|zone| (margin, zone)))
    {
        for (anchor, size) in [
            (across_top(), (0, 40)),
            (Anchor::BOTTOM | Anchor::RIGHT, (i32::MAX, i32::MAX)),
        ] {
            let mut world = World::new(10);
            let output = world.output(rect(0, 0, 1920, 1080));
            let mut state = layer_state(Level::Top, anchor, zone, size.0, size.1);
            state.margins = Margins {
                top: margin,
                right: margin,
                bottom: margin,
                left: margin,
            };
            world.layer(output, state);
            world.arrange(output);
            let work_area = world
                .desktop
                .output(output)
                .map(|output| output.work_area());
            assert!(work_area.is_some_and(|area| area.size.w >= 0 && area.size.h >= 0));
        }
    }
}
