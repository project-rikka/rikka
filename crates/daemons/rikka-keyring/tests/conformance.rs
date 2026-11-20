use std::io::Write as _;
use std::process::{Command, Stdio};

use rikka_secrets::Entry;
use tempfile::TempDir;

use crate::support::{FIRST, Running};

const SECRET_TOOL: &str = "secret-tool";
const GENERIC_SCHEMA: &str = "org.freedesktop.Secret.Generic";

fn secret_tool(arguments: &[&str], input: Option<&[u8]>) -> Vec<u8> {
    let mut child = Command::new(SECRET_TOOL)
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("secret-tool from libsecret should be on PATH");
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(input)
            .expect("secret-tool should read the secret");
    }
    let output = child.wait_with_output().expect("secret-tool should finish");
    assert!(
        output.status.success(),
        "secret-tool {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn generic(service: &str) -> Entry {
    Entry::new(GENERIC_SCHEMA)
        .attribute("service", service)
        .attribute("user", "alice")
}

#[test]
fn libsecret_and_rikka_clients_share_secrets() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs");

    secret_tool(
        &[
            "store",
            "--label=Stored by libsecret",
            "xdg:schema",
            GENERIC_SCHEMA,
            "service",
            "from-libsecret",
            "user",
            "alice",
        ],
        Some("パスワード 1".as_bytes()),
    );
    let read = zbus::block_on(running.secrets.lookup(&generic("from-libsecret")))
        .expect("lookup runs")
        .expect("libsecret's secret is found");
    assert_eq!(read.expose(), "パスワード 1".as_bytes());

    zbus::block_on(running.secrets.store(
        &generic("from-rikka"),
        "Stored by Rikka",
        b"second secret",
    ))
    .expect("secret stores");
    assert_eq!(
        secret_tool(&["lookup", "service", "from-rikka", "user", "alice"], None),
        b"second secret"
    );

    let listing = String::from_utf8(secret_tool(&["search", "--all", "user", "alice"], None))
        .expect("listing is text");
    assert!(listing.contains("label = Stored by libsecret"), "{listing}");
    assert!(listing.contains("label = Stored by Rikka"), "{listing}");

    secret_tool(
        &["clear", "service", "from-libsecret", "user", "alice"],
        None,
    );
    assert!(
        zbus::block_on(running.secrets.lookup(&generic("from-libsecret")))
            .expect("lookup runs")
            .is_none()
    );
}
