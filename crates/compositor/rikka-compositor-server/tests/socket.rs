use std::os::unix::net::{UnixListener, UnixStream};

use crate::common::{RawClient, Server};
use rikka_compositor_server::{ClientOptions, Error, SecurityContext};

#[test]
fn sockets_follow_libwayland_conventions() {
    let runtime = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", runtime.path()) };
    let server = Server::start(|_| {});

    let (first, second, clash) = server.call(|state| {
        let first = state.display.listen(None).unwrap();
        let second = state.display.listen(None).unwrap();
        let clash = state.display.listen(Some("wayland-0")).unwrap_err();
        let names = (
            first.name().to_owned(),
            second.name().to_owned(),
            matches!(clash, Error::SocketLocked(_)),
        );
        state.retained.push(Box::new(first));
        state.retained.push(Box::new(second));
        names
    });
    assert_eq!(
        (first.as_str(), second.as_str(), clash),
        ("wayland-0", "wayland-1", true)
    );
    assert!(runtime.path().join("wayland-0.lock").exists());

    let mut trusted =
        RawClient::new(UnixStream::connect(runtime.path().join("wayland-0")).unwrap());
    let (_, globals) = trusted.registry();
    assert!(
        globals
            .iter()
            .any(|(_, interface, _)| interface == "wl_fixes")
    );
    let pid = server.call(|state| state.display.clients()[0].credentials().pid);
    assert_eq!(pid, std::process::id() as i32);

    let sandbox = runtime.path().join("sandbox");
    let listener = UnixListener::bind(&sandbox).unwrap();
    server.call(move |state| {
        let context = SecurityContext {
            sandbox_engine: Some("flatpak".into()),
            app_id: Some("org.example.App".into()),
            instance_id: None,
        };
        let listener = state
            .display
            .accept_on(listener, ClientOptions::new().security_context(context))
            .unwrap();
        state.retained.push(Box::new(listener));
    });
    let mut sandboxed = RawClient::new(UnixStream::connect(&sandbox).unwrap());
    sandboxed.roundtrip();
    let clients = server.call(|state| {
        state
            .display
            .clients()
            .iter()
            .map(|client| {
                (
                    client.privileged(),
                    client
                        .security_context()
                        .and_then(|context| context.app_id.clone()),
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(
        clients,
        [(true, None), (false, Some("org.example.App".to_owned()))]
    );

    server.call(|state| state.retained.clear());
    assert!(!runtime.path().join("wayland-0").exists());
    assert!(!runtime.path().join("wayland-0.lock").exists());
    assert!(UnixStream::connect(&sandbox).is_err());
}
