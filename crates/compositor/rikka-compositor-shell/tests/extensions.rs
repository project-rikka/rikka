use crate::common::Server;
use crate::common::client::ForeignHandle;
use crate::common::map_toplevel;
use rikka_compositor_shell::ForeignToplevel;
use rikka_compositor_shell::{DecorationMode, DecorationPreference, ToplevelConfigure};
use rikka_wayland_client::{
    ext_foreign_toplevel_list_v1, wl_seat, xdg_activation_v1, xdg_wm_dialog_v1,
    zxdg_decoration_manager_v1, zxdg_exporter_v2, zxdg_importer_v2, zxdg_toplevel_decoration_v1,
};

#[test]
fn dialog_modal_until_destroyed() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let dialogs: xdg_wm_dialog_v1::XdgWmDialogV1 = client.global(1);
    let window = client.toplevel(&wm_base, 1);
    let dialog = dialogs.get_xdg_dialog(&window.toplevel);
    dialog.set_modal();
    client.roundtrip();
    server.expect("event: Modal");
    let id = server.call(|state| state.last_toplevel());
    assert!(server.call(move |state| state.shell.toplevel(id).unwrap().modal()));
    dialog.destroy();
    client.roundtrip();
    assert!(!server.call(move |state| state.shell.toplevel(id).unwrap().modal()));

    let _again = dialogs.get_xdg_dialog(&window.toplevel);
    client.roundtrip();
    dialogs.get_xdg_dialog(&window.toplevel);
    assert!(client.fails());
    server.expect("protocol error 0 on xdg_wm_dialog_v1");
}

#[test]
fn imported_toplevel_parents_foreign_window() {
    let server = Server::start();
    let mut exporting = server.connect();
    let wm_base = exporting.wm_base();
    let exported_window = map_toplevel(&server, &mut exporting, &wm_base, 1, (50, 50));
    let exporter: zxdg_exporter_v2::ZxdgExporterV2 = exporting.global(1);
    let exported = exporter.export_toplevel(&exported_window.window.surface);
    exporting.roundtrip();
    let handle = exporting.received.exported[0].clone();

    let mut importing = server.connect();
    let their_wm_base = importing.wm_base();
    let child = importing.toplevel(&their_wm_base, 2);
    let importer: zxdg_importer_v2::ZxdgImporterV2 = importing.global(1);
    let imported = importer.import_toplevel(&handle).with_key(7u32);
    imported.set_parent_of(&child.surface);
    importing.roundtrip();
    let parent = exported_window.id;
    let child_id = server.call(|state| state.last_toplevel());
    assert_eq!(
        server.call(move |state| state.shell.toplevel(child_id).unwrap().parent()),
        Some(parent)
    );

    exported.destroy();
    exporting.roundtrip();
    importing.roundtrip();
    assert_eq!(importing.received.imports_destroyed, vec![7]);
    assert_eq!(
        server.call(move |state| state.shell.toplevel(child_id).unwrap().parent()),
        None
    );

    let _unknown = importer.import_toplevel("nonsense").with_key(8u32);
    importing.roundtrip();
    assert_eq!(importing.received.imports_destroyed, vec![7, 8]);
}

#[test]
fn admitted_token_activates_once() {
    let server = Server::start();
    let mut client = server.connect();
    let activation: xdg_activation_v1::XdgActivationV1 = client.global(1);
    let seat: wl_seat::WlSeat = client.global(1);
    let surface = client.surface();
    let token = activation.get_activation_token();
    token.set_serial(5, &seat);
    token.set_app_id("rikka.files");
    token.set_surface(&surface);
    token.commit();
    client.roundtrip();
    server.expect("token requested with serial Some(5) and app id Some(\"rikka.files\")");
    let issued = client.received.tokens[0].clone();
    assert_eq!(issued.len(), 32);

    let target = client.surface();
    activation.activate(&issued, &target);
    client.roundtrip();
    server.expect("Activate {");
    activation.activate(&issued, &target);
    client.roundtrip();
    assert_eq!(server.call(|state| state.activated.len()), 1);
}

#[test]
fn refused_token_activates_nothing() {
    let server = Server::start();
    server.call(|state| state.admit_activation = false);
    let mut client = server.connect();
    let activation: xdg_activation_v1::XdgActivationV1 = client.global(1);
    let token = activation.get_activation_token();
    token.commit();
    client.roundtrip();
    let issued = client.received.tokens[0].clone();
    let target = client.surface();
    activation.activate(&issued, &target);
    client.roundtrip();
    assert!(server.call(|state| state.activated.is_empty()));
}

fn decorate(mode: Option<DecorationMode>) -> ToplevelConfigure {
    ToplevelConfigure {
        decoration: mode,
        ..ToplevelConfigure::default()
    }
}

#[test]
fn host_chooses_decoration_mode() {
    let server = Server::start();
    let mut client = server.connect();
    let wm_base = client.wm_base();
    let manager: zxdg_decoration_manager_v1::ZxdgDecorationManagerV1 = client.global(2);
    let window = client.toplevel(&wm_base, 1);
    let decoration = manager.get_toplevel_decoration(&window.toplevel);
    client.roundtrip();
    server.expect("event: Decoration");
    let id = server.call(|state| state.last_toplevel());
    let preference = server.call(move |state| state.shell.toplevel(id).unwrap().decoration());
    assert_eq!(preference, DecorationPreference::Unset);

    decoration.set_mode(zxdg_toplevel_decoration_v1::Mode::ServerSide);
    client.roundtrip();
    let preference = server.call(move |state| state.shell.toplevel(id).unwrap().decoration());
    assert_eq!(
        preference,
        DecorationPreference::Prefers(DecorationMode::ServerSide)
    );

    server.call(move |state| {
        state
            .shell
            .configure_toplevel(id, decorate(Some(DecorationMode::ClientSide)));
        state
            .shell
            .configure_toplevel(id, decorate(Some(DecorationMode::ClientSide)));
    });
    client.roundtrip();
    assert_eq!(client.received.decorations, vec![1]);

    decoration.unset_mode();
    client.roundtrip();
    server.call(move |state| {
        state
            .shell
            .configure_toplevel(id, decorate(Some(DecorationMode::ClientSide)))
    });
    client.roundtrip();
    assert_eq!(client.received.decorations, vec![1, 1]);

    decoration.destroy();
    client.roundtrip();
    let preference = server.call(move |state| state.shell.toplevel(id).unwrap().decoration());
    assert_eq!(preference, DecorationPreference::Absent);
}

fn info<'a>(title: &'a str, app_id: &'a str) -> ForeignToplevel<'a> {
    ForeignToplevel::Named {
        title: Some(title),
        app_id: Some(app_id),
    }
}

#[test]
fn published_toplevels_reach_lists() {
    let server = Server::start();
    let (first, first_identifier) = server.call(|state| {
        let id = state.shell.publish_toplevel(info("Files", "rikka.files"));
        (id, state.shell.published_identifier(id).unwrap().to_owned())
    });
    let mut client = server.connect();
    let list: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1 = client.global(1);
    client.roundtrip();
    assert_eq!(client.received.foreign.len(), 1);
    assert_eq!(
        client.received.foreign[0],
        ForeignHandle {
            id: client.received.foreign[0].id,
            identifier: Some(first_identifier),
            title: Some("Files".into()),
            app_id: Some("rikka.files".into()),
            done: 1,
            closed: false,
        }
    );

    let second = server.call(|state| {
        state
            .shell
            .publish_toplevel(info("Terminal", "rikka.terminal"))
    });
    client.roundtrip();
    assert_eq!(client.received.foreign.len(), 2);
    assert_eq!(
        client.received.foreign[1].title.as_deref(),
        Some("Terminal")
    );

    server.call(move |state| {
        state
            .shell
            .update_published_toplevel(first, info("Files — Home", "rikka.files"));
        state.shell.close_published_toplevel(second);
    });
    client.roundtrip();
    assert_eq!(
        client.received.foreign[0].title.as_deref(),
        Some("Files — Home")
    );
    assert_eq!(client.received.foreign[0].done, 2);
    assert!(client.received.foreign[1].closed);

    list.stop();
    client.roundtrip();
    server.call(|state| state.shell.publish_toplevel(info("Viewer", "rikka.viewer")));
    client.roundtrip();
    assert_eq!(client.received.foreign.len(), 2);
}
