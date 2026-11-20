use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use rikka_nested::{Event, Nested};

use crate::support::{self, Client};

#[test]
fn socket_lives_in_private_directory() {
    let (nested, events) = support::start();
    let socket = nested.socket().to_path_buf();
    let directory = socket.parent().unwrap().to_path_buf();
    assert!(socket.is_absolute());
    let mode = std::fs::metadata(&directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap();
    assert_eq!(directory.parent().unwrap(), std::path::Path::new(&runtime));
    let name = directory
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(name.starts_with("rikka-nested-") && name.len() >= "rikka-nested-".len() + 32);

    let mut client = Client::connect(&socket);
    assert_eq!(
        events.recv_timeout(support::TIMEOUT).ok(),
        Some(Event::ClientConnected {
            pid: std::process::id() as i32
        })
    );
    drop(nested);
    assert!(!socket.exists());
    assert!(!directory.exists());
    assert!(!client.alive());
}

#[test]
fn displays_never_share_socket() {
    let (first, _) = support::start();
    let (second, _) = support::start();
    assert_ne!(first.socket(), second.socket());
}

#[test]
fn stops_while_awaiting_gpu() {
    let nested = Nested::start().expect("the nested display starts");
    let socket = nested.socket().to_path_buf();
    let dropped = Instant::now();
    drop(nested);
    assert!(dropped.elapsed() < Duration::from_secs(1));
    assert!(!socket.exists());
}

#[test]
fn clients_see_unprivileged_globals_only() {
    let (nested, _events) = support::start();
    let client = Client::connect(nested.socket());
    for offered in [
        "wl_compositor",
        "wl_subcompositor",
        "wl_shm",
        "wl_seat",
        "wl_output",
        "zxdg_output_manager_v1",
        "xdg_wm_base",
        "xdg_activation_v1",
        "zwp_linux_dmabuf_v1",
        "wp_viewporter",
        "wp_fractional_scale_manager_v1",
        "wp_single_pixel_buffer_manager_v1",
        "wp_presentation",
        "wp_fifo_manager_v1",
        "wp_commit_timing_manager_v1",
        "zwp_idle_inhibit_manager_v1",
        "wp_cursor_shape_manager_v1",
        "zwp_pointer_gestures_v1",
        "zwp_relative_pointer_manager_v1",
        "zwp_pointer_constraints_v1",
        "zwp_text_input_manager_v3",
        "wl_data_device_manager",
        "zwp_primary_selection_device_manager_v1",
    ] {
        assert!(client.advertises(offered), "{offered} is missing");
    }
    for privileged in [
        "zwlr_layer_shell_v1",
        "ext_session_lock_manager_v1",
        "zwp_input_method_manager_v2",
        "zwp_virtual_keyboard_manager_v1",
        "zwlr_virtual_pointer_manager_v1",
        "ext_data_control_manager_v1",
        "ext_image_copy_capture_manager_v1",
        "ext_foreign_toplevel_list_v1",
    ] {
        assert!(!client.advertises(privileged), "{privileged} is offered");
    }
}

#[test]
fn client_disconnect_is_reported() {
    let (nested, events) = support::start();
    let client = Client::connect(nested.socket());
    assert!(matches!(
        events.recv_timeout(support::TIMEOUT).ok(),
        Some(Event::ClientConnected { .. })
    ));
    drop(client);
    let gone = std::iter::from_fn(|| events.recv_timeout(support::TIMEOUT).ok())
        .any(|event| matches!(event, Event::ClientGone { .. }));
    assert!(gone);
}
