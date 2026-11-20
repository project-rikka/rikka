use crate::common::{World, rect};
use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};
use rikka_compositor_shell::{PopupParent, Positioner};

fn beside(adjustment: ConstraintAdjustment) -> Positioner {
    Positioner {
        size: Size::new(200, 100),
        anchor_rect: rect(0, 0, 100, 100),
        anchor: Anchor::BottomRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: adjustment,
        ..Positioner::default()
    }
}

#[test]
fn offscreen_popup_flips_onto_output() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let window = world.window(rect(1800, 100, 100, 100));
    let menu = world.popup(
        PopupParent::Toplevel(world.toplevel_of(window)),
        rect(0, 0, 0, 0),
    );

    assert_eq!(
        world
            .desktop
            .constrain_popup(menu, &beside(ConstraintAdjustment::FLIP_X)),
        Ok(rect(-200, 100, 200, 100))
    );
    assert_eq!(
        world
            .desktop
            .constrain_popup(menu, &beside(ConstraintAdjustment::empty())),
        Ok(rect(100, 100, 200, 100))
    );
}

#[test]
fn removing_popup_removes_descendants() {
    let mut world = World::new(10);
    world.output(rect(0, 0, 1920, 1080));
    let window = world.window(rect(100, 100, 400, 300));
    let outer = world.popup(
        PopupParent::Toplevel(world.toplevel_of(window)),
        rect(0, 0, 50, 50),
    );
    let inner = world.popup(PopupParent::Popup(outer), rect(0, 0, 20, 20));
    let sibling = world.popup(
        PopupParent::Toplevel(world.toplevel_of(window)),
        rect(0, 0, 50, 50),
    );

    assert!(world.desktop.remove_popup(outer).is_some());
    assert!(world.desktop.popup(inner).is_none());
    assert!(world.desktop.popup(sibling).is_some());

    world.desktop.remove_window(window);
    assert!(world.desktop.popup(sibling).is_none());
}
