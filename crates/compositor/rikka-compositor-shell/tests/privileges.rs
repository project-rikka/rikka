use crate::common::Server;
use rikka_compositor_server::{ClientOptions, SecurityContext};
use rikka_wayland_client::xwayland_shell_v1;

const PRIVILEGED: [&str; 3] = [
    "zwlr_layer_shell_v1",
    "ext_session_lock_manager_v1",
    "ext_foreign_toplevel_list_v1",
];

const EVERYONE: [&str; 10] = [
    "xdg_wm_base",
    "zxdg_decoration_manager_v1",
    "xdg_activation_v1",
    "zxdg_exporter_v2",
    "zxdg_importer_v2",
    "xdg_wm_dialog_v1",
    "xdg_toplevel_icon_manager_v1",
    "xdg_toplevel_tag_manager_v1",
    "xdg_toplevel_drag_manager_v1",
    "xdg_system_bell_v1",
];

#[test]
fn unprivileged_clients_see_shared_shell() {
    let server = Server::start();
    for options in [
        ClientOptions::new().unprivileged(),
        ClientOptions::new().security_context(SecurityContext {
            sandbox_engine: Some("flatpak".into()),
            app_id: Some("org.example.App".into()),
            instance_id: None,
        }),
    ] {
        let client = server.connect_with(options);
        for interface in PRIVILEGED {
            assert!(!client.advertises(interface), "{interface} leaked");
        }
        for interface in EVERYONE {
            assert!(client.advertises(interface), "{interface} is missing");
        }
        assert!(!client.advertises("xwayland_shell_v1"));
    }
}

#[test]
fn xwayland_shell_is_xwayland_only() {
    let server = Server::start();
    let client = server.connect_xwayland();
    assert!(client.advertises("xwayland_shell_v1"));
}

const SERIAL: u64 = (7 << 32) | 42;

#[test]
fn committed_serial_associates_surface() {
    let server = Server::start();
    let mut client = server.connect_xwayland();
    let shell: xwayland_shell_v1::XwaylandShellV1 = client.global(1);
    let surface = client.surface();
    let xwayland_surface = shell.get_xwayland_surface(&surface);
    xwayland_surface.set_serial(42, 7);
    client.roundtrip();
    assert_eq!(
        server.call(|state| state.shell.surface_for_xwayland_serial(SERIAL)),
        None
    );

    surface.commit();
    client.roundtrip();
    server.expect("XwaylandAssociated { surface: SurfaceId(");
    let associated = server.call(|state| state.shell.surface_for_xwayland_serial(SERIAL));
    assert!(associated.is_some());

    surface.destroy();
    client.roundtrip();
    assert_eq!(
        server.call(|state| state.shell.surface_for_xwayland_serial(SERIAL)),
        None
    );
}
