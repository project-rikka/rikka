use std::collections::BTreeMap;

use rikka_age::Encryptor;
use rikka_backup::{Base, Bundle, Configured, Credential, Error, File, Origin, Preference};
use rikka_nix_edit::Expr;
use rikka_nixos_layout::Module;
use zeroize::Zeroizing;

const PASSPHRASE: &str = "a long passphrase";

const SETTINGS: &str = "{ ... }:
{
  rikka.users.alice.settings.\"rikka.windows\".layout = \"tiling\";
}
";

fn fast() -> Encryptor {
    Encryptor::with_passphrase(PASSPHRASE).work_factor(4)
}

fn bundle() -> Bundle {
    Bundle {
        origin: Origin {
            host: "old".to_owned(),
            user: "alice".to_owned(),
            home: "/home/alice".to_owned(),
            created: 1_700_000_000,
        },
        modules: vec![Configured {
            module: Module::Settings,
            text: SETTINGS.to_owned(),
        }],
        preferences: vec![Preference {
            domain: "rikka.pointer".to_owned(),
            name: "natural-scrolling".to_owned(),
            value: serde_json::Value::Bool(true),
        }],
        files: vec![
            File::new(
                Base::State,
                "discord/accounts.json",
                Zeroizing::new(b"{\"accounts\":[]}".to_vec()),
            )
            .unwrap(),
        ],
        credentials: vec![Credential {
            label: "Discord sign-in".to_owned(),
            attributes: BTreeMap::from([("folder".to_owned(), "/home/alice/x".to_owned())]),
            secret: Zeroizing::new(vec![0, 1, 2, 255]),
        }],
    }
}

fn sealed_json(content: &str) -> Vec<u8> {
    let json = format!("{{\"format\":\"rikka-backup\",\"version\":1,\"content\":{content}}}");
    let mut writer = fast().wrap(Vec::new()).unwrap();
    std::io::Write::write_all(&mut writer, json.as_bytes()).unwrap();
    writer.finish().unwrap()
}

#[test]
fn round_trips_through_a_passphrase() {
    let sealed = bundle().seal_with(fast()).unwrap();
    assert!(sealed.starts_with(b"age-encryption.org/v1\n-> scrypt "));
    assert_eq!(
        Bundle::open(sealed.as_slice(), PASSPHRASE).unwrap(),
        bundle()
    );
}

#[test]
fn refuses_the_wrong_passphrase() {
    let sealed = bundle().seal_with(fast()).unwrap();
    let error = Bundle::open(sealed.as_slice(), "another").unwrap_err();
    assert!(matches!(error, Error::Age(rikka_age::Error::Passphrase)));
}

#[test]
fn refuses_damaged_contents() {
    let origin = r#""origin":{"host":"h","user":"u","home":"/home/u","created":0}"#;
    let cases = [
        (
            "machine module",
            format!(
                r#"{{{origin},"modules":[{{"module":"boot","text":"{{ }}"}}],"preferences":[],"files":[],"credentials":[]}}"#
            ),
        ),
        (
            "escaping path",
            format!(
                r#"{{{origin},"modules":[],"preferences":[],"files":[{{"base":"state","path":"../x","contents":""}}],"credentials":[]}}"#
            ),
        ),
        (
            "absolute path",
            format!(
                r#"{{{origin},"modules":[],"preferences":[],"files":[{{"base":"data","path":"/etc/x","contents":""}}],"credentials":[]}}"#
            ),
        ),
        (
            "repeated file",
            format!(
                r#"{{{origin},"modules":[],"preferences":[],"files":[{{"base":"data","path":"x","contents":""}},{{"base":"data","path":"x","contents":""}}],"credentials":[]}}"#
            ),
        ),
    ];
    for (name, content) in cases {
        let error = Bundle::open(sealed_json(&content).as_slice(), PASSPHRASE).unwrap_err();
        assert!(matches!(error, Error::Invalid(_)), "{name}: {error}");
    }
}

#[test]
fn moves_declarations_to_the_restoring_account() {
    let from_bundle = bundle().documents_for("bob").unwrap();
    let from_repository = Configured::documents(&bundle().modules, None, "bob").unwrap();
    for documents in [from_bundle, from_repository] {
        let settings = &documents[&Module::Settings];
        assert_eq!(
            settings.get::<Expr>(["rikka", "users", "alice"]).unwrap(),
            None
        );
        assert_eq!(
            settings
                .get::<String>([
                    "rikka",
                    "users",
                    "bob",
                    "settings",
                    "rikka.windows",
                    "layout"
                ])
                .unwrap()
                .as_deref(),
            Some("tiling")
        );
    }
}
