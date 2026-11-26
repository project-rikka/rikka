use std::process::Command;

use rikka_backup::Configured;
use rikka_nix::Module;
use settings_backup::{Identity, Published, Repository};

fn has_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn modules(layout: &str) -> Vec<Configured> {
    vec![
        Configured {
            module: Module::Packages,
            text: "{ pkgs, ... }:\n{ }\n".to_owned(),
        },
        Configured {
            module: Module::Settings,
            text: format!(
                "{{ ... }}:\n{{\n  rikka.settings.\"rikka.windows\".layout = \"{layout}\";\n}}\n"
            ),
        },
    ]
}

#[test]
fn publishes_the_configuration_from_every_computer_onto_one_branch() {
    if !has_git() {
        return;
    }
    let remote_folder = tempfile::tempdir().unwrap();
    let remote = remote_folder.path().join("configuration.git");
    let status = Command::new("git")
        .args(["init", "--quiet", "--bare", "--initial-branch=main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success());
    let url = remote.to_str().unwrap();
    let identity = Identity {
        name: "Rikka".to_owned(),
        email: "rikka@example.com".to_owned(),
    };

    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let laptop = Repository::new(first.path().join("clone"));
    let desktop = Repository::new(second.path().join("clone"));
    assert_eq!(
        laptop
            .publish(url, &modules("tiling"), "laptop", &identity)
            .unwrap(),
        Published::Pushed
    );
    assert_eq!(
        laptop
            .publish(url, &modules("tiling"), "laptop", &identity)
            .unwrap(),
        Published::Unchanged
    );
    assert_eq!(
        desktop
            .publish(url, &modules("floating"), "desktop", &identity)
            .unwrap(),
        Published::Pushed
    );

    let checkout = tempfile::tempdir().unwrap();
    let fetched = Repository::fetch(url, &checkout.path().join("checkout")).unwrap();
    assert_eq!(fetched, modules("floating"));
}
