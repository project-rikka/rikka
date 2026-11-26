use std::collections::BTreeMap;

use rikka_backup::{Credential, Origin};
use zeroize::Zeroizing;

fn origin() -> Origin {
    Origin {
        host: "old".to_owned(),
        user: "alice".to_owned(),
        home: "/home/alice".to_owned(),
        created: 0,
    }
}

#[test]
fn rebases_values_inside_the_old_home() {
    let cases = [
        ("/home/alice", "/home/bob"),
        (
            "/home/alice/.local/state/rikka/discord",
            "/home/bob/.local/state/rikka/discord",
        ),
        ("/home/alicesmith/notes", "/home/alicesmith/notes"),
        (
            "https://example.com/home/alice",
            "https://example.com/home/alice",
        ),
        ("token", "token"),
    ];
    for (value, expected) in cases {
        assert_eq!(origin().rebase(value, "/home/bob"), expected, "{value}");
    }
}

#[test]
fn rebases_every_credential_attribute() {
    let credential = Credential {
        label: "Discord".to_owned(),
        attributes: BTreeMap::from([
            (
                "folder".to_owned(),
                "/home/alice/.local/state/rikka/discord/accounts/1".to_owned(),
            ),
            ("xdg:schema".to_owned(), "os.rikka.Discord".to_owned()),
        ]),
        secret: Zeroizing::new(b"token".to_vec()),
    };
    let rebased = credential.rebased(&origin(), "/home/bob/");
    assert_eq!(
        rebased["folder"],
        "/home/bob/.local/state/rikka/discord/accounts/1"
    );
    assert_eq!(rebased["xdg:schema"], "os.rikka.Discord");
}
