use rikka_session::remote::{self, Address, Codec, Encoding, Reach, RemoteStatus, Serving, Viewer};

use crate::support::{Recorder, Session};

fn watched() -> RemoteStatus {
    RemoteStatus {
        serving: Some(Serving {
            port: 47900,
            addresses: vec![
                Address {
                    address: "localhost:47900".to_owned(),
                    reach: Reach::Local,
                },
                Address {
                    address: "100.64.0.9:47900".to_owned(),
                    reach: Reach::Tailnet,
                },
            ],
            fingerprint: "ab12:cd34".to_owned(),
        }),
        viewer: Some(Viewer {
            address: "100.64.0.2:51000".to_owned(),
            device: "laptop".to_owned(),
            since: 1_700_000_000,
            encoding: Some(Encoding {
                codec: Codec::H265,
                width: 1920,
                height: 1080,
                bitrate: 12_000_000,
                encoder: "GPU".to_owned(),
            }),
        }),
        problem: None,
    }
}

#[test]
fn remote_status_names_addresses_fingerprint_and_viewer() {
    let mut session = Session::start();
    let service = Recorder::new(remote::PROTOCOL)
        .answer(remote::STATUS, watched())
        .accept(remote::DISCONNECT);
    session.serve("remote", service.clone());

    let shown = session.expect_success(&["remote", "status"]);
    let rows: Vec<Vec<&str>> = shown
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(rows[0], ["serving", "localhost:47900"], "{shown}");
    assert_eq!(rows[1], ["100.64.0.9:47900"], "{shown}");
    assert_eq!(rows[2], ["fingerprint", "ab12:cd34"], "{shown}");
    assert!(
        rows.contains(&vec!["viewer", "laptop", "(100.64.0.2:51000)"]),
        "{shown}"
    );
    assert!(
        rows.contains(&vec![
            "picture",
            "H.265",
            "1920x1080",
            "12.0",
            "Mbit/s",
            "via",
            "GPU"
        ]),
        "{shown}"
    );

    let reported: RemoteStatus =
        serde_json::from_str(&session.expect_success(&["remote", "status", "--json"])).unwrap();
    assert_eq!(reported, watched());

    session.expect_success(&["remote", "disconnect"]);
    let methods: Vec<String> = service
        .calls()
        .into_iter()
        .map(|recorded| recorded.method)
        .filter(|method| method != remote::STATUS)
        .collect();
    assert_eq!(methods, [remote::DISCONNECT]);
}

#[test]
fn remote_commands_fail_without_service() {
    let session = Session::start();
    let failed = session.expect_failure(&["remote", "status"]);
    assert!(failed.contains(remote::CAPABILITY), "{failed}");
}
