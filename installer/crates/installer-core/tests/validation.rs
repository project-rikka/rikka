use installer_core::{Answers, Catalog, GpuVendor, Secret, Step, validate_step};

use crate::support::{answers, gpu};

fn account(hostname: &str, username: &str) -> Answers {
    Answers {
        hostname: hostname.to_owned(),
        username: username.to_owned(),
        password_confirm: Secret::from("correct horse battery"),
        ..answers()
    }
}

#[test]
fn validates_hostnames_and_usernames() {
    for (hostname, username, accepted) in [
        ("nixos", "eldodebug", true),
        ("rikka-01", "_service", true),
        ("", "eldodebug", false),
        ("-nixos", "eldodebug", false),
        ("nixos.local", "eldodebug", false),
        ("ホスト", "eldodebug", false),
        ("nixos", "1st", false),
        ("nixos", "Eldo", false),
        ("nixos", "root", false),
    ] {
        assert_eq!(
            validate_step(&account(hostname, username), Step::Account).is_empty(),
            accepted,
            "{hostname} {username}"
        );
    }
}

#[test]
fn account_takes_any_matching_password() {
    for (password, confirm, accepted) in [
        ("a", "a", true),
        ("1234", "1234", true),
        ("correct horse battery", "correct horse batter", false),
        ("", "", false),
    ] {
        let answers = Answers {
            password: Secret::from(password),
            password_confirm: Secret::from(confirm),
            ..answers()
        };
        assert_eq!(
            validate_step(&answers, Step::Account).is_empty(),
            accepted,
            "{password:?} {confirm:?}"
        );
    }
}

#[test]
fn nvidia_driver_requires_unfree() {
    let nvidia = Answers {
        nvidia: true,
        gpus: vec![gpu(GpuVendor::Nvidia, 0x2882, "nouveau")],
        ..answers()
    };
    assert_eq!(validate_step(&nvidia, Step::System).len(), 1);
    assert!(
        validate_step(
            &Answers {
                allow_unfree: true,
                ..nvidia
            },
            Step::System
        )
        .is_empty()
    );
}

#[test]
fn summary_lists_missing_answers() {
    let answers = Answers {
        username: String::new(),
        ..answers()
    };
    let problems = validate_step(&answers, Step::Summary);
    assert!(problems.iter().any(|problem| problem.step == Step::Account));
}

#[test]
fn apps_require_catalog() {
    assert!(validate_step(&answers(), Step::Apps).is_empty());
    let offline = Answers {
        catalog: Catalog::default(),
        ..answers()
    };
    assert_eq!(validate_step(&offline, Step::Apps).len(), 1);
}
