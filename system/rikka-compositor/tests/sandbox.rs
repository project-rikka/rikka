use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};

use rikka_compositor::Extent;
use rikka_test_display::Client;
use rikka_wayland_client::wp_security_context_manager_v1::WpSecurityContextManagerV1;

use crate::common::{Harness, interfaces};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const PRIVILEGED: [&str; 9] = [
    "wp_security_context_manager_v1",
    "zwlr_layer_shell_v1",
    "ext_session_lock_manager_v1",
    "ext_data_control_manager_v1",
    "ext_foreign_toplevel_list_v1",
    "ext_image_copy_capture_manager_v1",
    "zwp_virtual_keyboard_manager_v1",
    "zwlr_virtual_pointer_manager_v1",
    "ext_workspace_manager_v1",
];

#[test]
fn sandboxed_client_sees_no_privileged_globals() {
    let harness = Harness::full(EXTENT, &[]);
    let trusted = harness.globals();
    for privileged in PRIVILEGED {
        assert!(
            trusted.iter().any(|global| global == privileged),
            "trusted client sees {privileged}"
        );
    }

    let socket = harness.socket().with_file_name("sandbox-0");
    let listener = UnixListener::bind(&socket).unwrap();
    let (close, _open) = std::io::pipe().unwrap();

    let mut trusted_client = Client::<()>::connect_to(harness.socket(), |_| {});
    let manager: WpSecurityContextManagerV1 = trusted_client.global(1);
    let context = manager.create_listener(listener.as_fd(), close.as_fd());
    context.set_sandbox_engine("org.rikka.test");
    context.set_app_id("org.rikka.Sandboxed");
    context.commit();
    trusted_client.roundtrip();

    let sandboxed = interfaces(&Client::<()>::connect(
        UnixStream::connect(&socket).unwrap(),
        |_| {},
    ));
    assert!(sandboxed.iter().any(|global| global == "wl_compositor"));
    for privileged in PRIVILEGED {
        assert!(
            !sandboxed.iter().any(|global| global == privileged),
            "sandboxed client does not see {privileged}"
        );
    }
}
