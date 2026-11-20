use std::path::PathBuf;

use rikka_seam::net::{SOCKET_ENV, session_sockets, socket_path, socket_path_beside};
use rikka_seam::{CapabilityId, Version};

#[test]
fn routing_key_requires_lowercase_reverse_dns() {
    for name in ["rikka.wallpaper", "com.example.notes-2", "rikka"] {
        assert!(CapabilityId::from(name).is_valid(), "refused {name:?}");
    }
    for name in [
        "",
        "rikka.Wallpaper",
        "rikka/wallpaper",
        "rikka wallpaper",
        ".wallpaper",
        "wallpaper.",
        "rikka..wallpaper",
    ] {
        assert!(!CapabilityId::from(name).is_valid(), "accepted {name:?}");
    }
    assert!(!CapabilityId::from("k".repeat(129)).is_valid());
}

#[test]
fn version_satisfies_compatible_requirement() {
    let provided = Version::new(1, 7);
    assert!(provided.satisfies(Version::new(1, 2)));
    assert!(provided.satisfies(Version::new(1, 7)));
    assert!(!provided.satisfies(Version::new(1, 8)));
    assert!(!provided.satisfies(Version::new(2, 0)));
    assert!(!provided.satisfies(Version::new(0, 1)));
}

#[test]
fn session_socket_resolution() {
    let runtime = tempfile::tempdir().expect("temp dir should be created");
    let session = runtime.path().join("rikka").join("wayland-9");
    std::fs::create_dir_all(&session).expect("session dir should be created");
    for name in ["seam.sock", "daemon.sock", "session.env"] {
        std::fs::write(session.join(name), b"").expect("file should be written");
    }

    unsafe {
        std::env::set_var(SOCKET_ENV, "/run/somewhere/else.sock");
    }
    assert_eq!(
        socket_path(),
        Some(PathBuf::from("/run/somewhere/else.sock"))
    );

    unsafe {
        std::env::remove_var(SOCKET_ENV);
        std::env::set_var("XDG_RUNTIME_DIR", runtime.path());
        std::env::set_var("WAYLAND_DISPLAY", "wayland-9");
    }
    assert_eq!(socket_path(), Some(session.join("seam.sock")));
    assert_eq!(
        socket_path_beside("shell"),
        Some(session.join("shell.sock"))
    );
    assert_eq!(
        session_sockets(),
        [session.join("seam.sock"), session.join("daemon.sock")]
    );

    unsafe {
        std::env::remove_var("XDG_RUNTIME_DIR");
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    assert!(socket_path().is_none());
    assert!(session_sockets().is_empty());
}
