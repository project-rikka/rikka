use crate::common::{Server, map_toplevel};
use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_shell::PopupParent;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};

#[test]
fn popup_placed_by_positioner() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let parent = map_toplevel(&server, &mut client, &wm_base, 1, (200, 200));
    let positioner = client.positioner(&wm_base, (80, 30), (150, 10, 20, 20));
    positioner.set_anchor(Anchor::Right);
    positioner.set_gravity(Gravity::Right);
    positioner.set_constraint_adjustment(ConstraintAdjustment::FLIP_X);
    let surface = client.surface();
    let xdg_surface = wm_base.get_xdg_surface(&surface).with_key(2u32);
    let _popup = xdg_surface
        .get_popup(Some(&parent.window.xdg_surface), &positioner)
        .with_key(2u32);
    surface.commit();
    client.roundtrip();
    server.expect("event: Created");
    server.expect("event: InitialCommit");

    let toplevel = parent.id;
    let (id, root, placed) = server.call(move |state| {
        let id = state.last_popup();
        let popup = state.shell.popup(id).unwrap();
        let constraint = Rect::new(Point::new(0, 0), Size::new(200, 200));
        let placed = popup.positioner().geometry(constraint);
        (id, state.shell.popup_root(id), placed)
    });
    assert_eq!(root, Some(PopupParent::Toplevel(toplevel)));
    assert_eq!(placed, Rect::new(Point::new(70, 5), Size::new(80, 30)));
    server.call(move |state| state.shell.configure_popup(id, placed));
    client.roundtrip();
    assert_eq!(client.received.popup_configures, vec![(2, 70, 5, 80, 30)]);

    xdg_surface.ack_configure(client.received.last_configure(2));
    let buffer = client.canvas(80, 30, Format::Argb8888);
    surface.attach(Some(&buffer.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    server.expect("event: Configured");
    server.expect("event: Mapped");
    let geometry = server.call(move |state| state.shell.popup(id).unwrap().geometry());
    assert_eq!(geometry, Some(placed));
}

#[test]
fn dismissal_closes_children_topmost_first() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let parent = map_toplevel(&server, &mut client, &wm_base, 1, (100, 100));
    let positioner = client.positioner(&wm_base, (10, 10), (0, 0, 1, 1));
    let menu_surface = client.surface();
    let menu = wm_base.get_xdg_surface(&menu_surface).with_key(2u32);
    let _menu_popup = menu
        .get_popup(Some(&parent.window.xdg_surface), &positioner)
        .with_key(2u32);
    let submenu_surface = client.surface();
    let submenu = wm_base.get_xdg_surface(&submenu_surface).with_key(3u32);
    let _submenu_popup = submenu.get_popup(Some(&menu), &positioner).with_key(3u32);
    client.roundtrip();
    let (menu_id, submenu_id) = server.call(|state| (state.popups[0], state.popups[1]));
    let root = server.call(move |state| state.shell.popup_root(submenu_id));
    assert_eq!(root, Some(PopupParent::Toplevel(parent.id)));
    assert_eq!(
        server.call(move |state| state.shell.popup(submenu_id).unwrap().parent()),
        Some(PopupParent::Popup(menu_id))
    );

    server.call(move |state| state.shell.dismiss_popup(menu_id));
    client.roundtrip();
    assert_eq!(client.received.popups_done, vec![3, 2]);
}

#[test]
fn parent_destruction_dismisses_popups() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let parent = client.toplevel(&wm_base, 1);
    let positioner = client.positioner(&wm_base, (10, 10), (0, 0, 1, 1));
    let surface = client.surface();
    let xdg_surface = wm_base.get_xdg_surface(&surface).with_key(2u32);
    let _popup = xdg_surface
        .get_popup(Some(&parent.xdg_surface), &positioner)
        .with_key(2u32);
    client.roundtrip();
    parent.toplevel.destroy();
    client.roundtrip();
    assert_eq!(client.received.popups_done, vec![2]);
}

#[test]
fn extreme_positioner_places_without_overflow() {
    let cases = [
        ((i32::MAX, i32::MAX, 1, 1), (0, 0), Anchor::BottomRight),
        (
            (i32::MIN, i32::MIN, 1, 1),
            (i32::MIN, i32::MIN),
            Anchor::TopLeft,
        ),
        (
            (0, 0, i32::MAX, i32::MAX),
            (i32::MAX, i32::MAX),
            Anchor::BottomRight,
        ),
        (
            (i32::MAX, 0, i32::MAX, 1),
            (i32::MAX, i32::MIN),
            Anchor::Right,
        ),
    ];
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let parent = map_toplevel(&server, &mut client, &wm_base, 1, (200, 200));
    for (anchor_rect, offset, anchor) in cases {
        for gravity in [Gravity::TopLeft, Gravity::None, Gravity::BottomRight] {
            let positioner = client.positioner(&wm_base, (i32::MAX, i32::MAX), anchor_rect);
            positioner.set_offset(offset.0, offset.1);
            positioner.set_anchor(anchor);
            positioner.set_gravity(gravity);
            positioner.set_constraint_adjustment(ConstraintAdjustment::all());
            let surface = client.surface();
            let xdg_surface = wm_base.get_xdg_surface(&surface).with_key(2u32);
            let popup = xdg_surface
                .get_popup(Some(&parent.window.xdg_surface), &positioner)
                .with_key(2u32);
            client.roundtrip();
            let constraint = Rect::new(Point::new(0, 0), Size::new(200, 200));
            let (unconstrained, placed) = server.call(move |state| {
                let positioner = *state.shell.popup(state.last_popup()).unwrap().positioner();
                (positioner.placement(), positioner.geometry(constraint))
            });
            assert_eq!(unconstrained.size, Size::new(i32::MAX, i32::MAX));
            assert!(placed.size.w > 0 && placed.size.h > 0);
            popup.destroy();
            xdg_surface.destroy();
            surface.destroy();
        }
    }
    assert!(!client.fails());
}
