use rikka_seam::Version;
use rikka_session::protocol::Protocol;

use crate::support::{Output, Recorder, Session, rikka};

const ECHO: Protocol = Protocol::new("test.echo", Version::new(1, 0));

#[test]
fn capabilities_merge_sockets_and_calls_route() {
    let mut session = Session::start();
    session.serve(
        "daemon",
        Recorder::new(ECHO).answer("say", "beside").accept("hush"),
    );

    let listed = session.expect_success(&["seam", "capabilities"]);
    assert!(listed.contains("rikka.settings"), "{listed}");
    assert!(listed.contains("test.echo 1.0"), "{listed}");
    assert!(
        listed
            .lines()
            .any(|line| line.split_whitespace().eq(["method", "say"])),
        "{listed}"
    );
    assert_eq!(
        listed.matches("rikka.seam").count(),
        1,
        "the seam's own account of itself is listed once: {listed}"
    );

    assert_eq!(
        session.expect_success(&["seam", "call", "test.echo", "hush"]),
        ""
    );
    assert_eq!(
        session
            .expect_success(&["--json", "seam", "call", "test.echo", "hush"])
            .trim(),
        "null"
    );

    assert_eq!(
        session
            .expect_success(&["seam", "call", "test.echo", "say"])
            .trim(),
        "\"beside\""
    );
    assert_eq!(
        session
            .expect_success(&[
                "seam",
                "call",
                "rikka.settings",
                "get",
                r#"{"domain":"rikka.input","key":"close-window"}"#,
            ])
            .trim(),
        "\"Super+Q\"",
        "settings still come from the compositor"
    );

    let manifests: serde_json::Value =
        serde_json::from_str(&session.expect_success(&["--json", "seam", "capabilities"])).unwrap();
    assert!(
        manifests
            .as_array()
            .unwrap()
            .iter()
            .any(|manifest| manifest["capability"] == "test.echo"),
        "{manifests}"
    );
}

#[test]
fn rejects_unknown_capability_and_invalid_json() {
    let session = Session::start();

    let unserved = session.expect_failure(&["seam", "call", "test.nothing", "at-all"]);
    assert!(unserved.contains("test.nothing"), "{unserved}");

    let not_json = session.expect_failure(&["seam", "call", "rikka.settings", "get", "{domain"]);
    assert!(not_json.contains("invalid JSON"), "{not_json}");
}

#[test]
fn commands_fail_without_session() {
    let runtime = tempfile::tempdir().unwrap();
    let expect_failure = Output::run(
        rikka(&["seam", "capabilities"])
            .env_remove(rikka_seam::net::SOCKET_ENV)
            .env("XDG_RUNTIME_DIR", runtime.path())
            .env("WAYLAND_DISPLAY", "wayland-7"),
    )
    .expect_failure(&["seam", "capabilities"]);
    assert!(
        expect_failure.contains("no Rikka session"),
        "{expect_failure}"
    );
}
