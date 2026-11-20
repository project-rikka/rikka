use installer_core::{
    Answers, Channel, Filesystem, GpuVendor, Kernel, LOCALES, PACKAGES_FLAKE, SECRETS_DIR, Secret,
    Swap,
};
use rikka_nix_edit::{AttrPath, Document, Expr};

use crate::support::{answers, file, generated, gpu};

fn variations() -> Vec<Answers> {
    let buses = [
        Vec::new(),
        vec![
            gpu(GpuVendor::Intel, 0x5917, "i915"),
            gpu(GpuVendor::Nvidia, 0x2882, "nouveau"),
        ],
        vec![
            gpu(GpuVendor::Amd, 0x73FF, "amdgpu"),
            gpu(GpuVendor::Nvidia, 0x1B80, "nouveau"),
        ],
    ];
    LOCALES
        .iter()
        .enumerate()
        .map(|(index, locale)| {
            let flag = |bit: usize| index & (1 << bit) != 0;
            let encrypt = flag(0);
            Answers {
                locale,
                encrypt,
                passphrase: Secret::from("a long passphrase"),
                english_messages: flag(1),
                openssh: flag(2),
                ssh_password_auth: flag(2) && encrypt,
                swap: if flag(3) {
                    Swap::None
                } else {
                    Swap::Zram { percent: 50 }
                },
                filesystem: if encrypt {
                    Filesystem::Ext4
                } else {
                    Filesystem::Btrfs
                },
                sudo_rs: flag(1),
                allow_unfree: flag(2) || encrypt,
                nvidia: encrypt,
                gpus: buses[index % buses.len()].clone(),
                kernel: Kernel::ALL[index % Kernel::ALL.len()],
                full_name: r#"Eldo "the ${escape}" Debug"#.to_owned(),
                ..answers()
            }
        })
        .collect()
}

#[test]
fn all_answers_render_nix() {
    for answers in variations() {
        for file in generated(&answers) {
            if file.path.ends_with(".nix") {
                assert!(
                    Document::parse(file.contents.as_str()).is_ok(),
                    "{}\n{}",
                    file.path,
                    file.contents
                );
            }
        }
    }
}

#[test]
fn rendered_nix_is_nixfmt_clean() {
    let everything = Answers {
        encrypt: true,
        english_messages: true,
        openssh: true,
        ssh_password_auth: true,
        set_root_password: true,
        root_password: Secret::from("another long password"),
        sudo_rs: true,
        allow_unfree: true,
        nvidia: true,
        gpus: vec![
            gpu(GpuVendor::Amd, 0x73FF, "amdgpu"),
            gpu(GpuVendor::Nvidia, 0x1B80, "nouveau"),
        ],
        full_name: r#"Eldo "the ${escape}" Debug"#.to_owned(),
        ..answers()
    };
    let plain = Answers {
        locale: installer_core::Locale::find("en_US.UTF-8")
            .expect("entry should be in the catalog"),
        filesystem: Filesystem::Ext4,
        swap: Swap::None,
        kernel: Kernel::Default,
        ..answers()
    };
    for answers in [answers(), everything, plain] {
        for file in generated(&answers) {
            if file.path.ends_with(".nix") {
                rikka_test_nixfmt::assert_formatted(&file.contents);
            }
        }
    }
}

#[test]
fn escapes_unvalidated_hostname() {
    let answers = Answers {
        hostname: r#"a" ; imports = [ ./evil.nix ]; x = ""#.to_owned(),
        ..answers()
    };
    let files = generated(&answers);
    let flake = Document::parse(file(&files, "flake.nix")).expect("output should parse as Nix");
    assert_eq!(
        flake.get::<String>("description"),
        Ok(Some(format!(
            "NixOS configuration for {}",
            answers.hostname
        )))
    );
    assert_eq!(
        flake.paths().expect("document should be an attribute set"),
        [
            "description",
            "inputs.rikka.url",
            "inputs.nixpkgs.follows",
            "outputs",
        ]
        .map(|path| AttrPath::parse(path).expect("path"))
    );
}

#[test]
fn root_login_disabled_by_default() {
    let files = generated(&answers());
    assert!(!file(&files, "configuration.nix").contains("users.users.root"));
    assert!(!files.iter().any(|file| file.path.ends_with("/root")));

    let files = generated(&Answers {
        set_root_password: true,
        root_password: Secret::from("another long password"),
        ..answers()
    });
    assert!(file(&files, "configuration.nix").contains(&format!(
        r#"users.users.root.hashedPasswordFile = "{SECRETS_DIR}/root";"#
    )));
}

#[test]
fn password_hashes_stay_private() {
    let files = generated(&Answers {
        set_root_password: true,
        root_password: Secret::from("another long password"),
        root_password_confirm: Secret::from("another long password"),
        ..answers()
    });

    let secrets: Vec<_> = files.iter().filter(|file| file.secret).collect();
    assert_eq!(secrets.len(), 2);
    for file in secrets {
        assert!(
            file.path.starts_with(&format!("{SECRETS_DIR}/")),
            "{}",
            file.path
        );
        assert_eq!(file.mode, 0o400);
        assert!(file.contents.starts_with("$6$rounds="));
        assert!(!file.contents.contains("correct horse"));
    }
    for file in files.iter().filter(|file| !file.secret) {
        assert!(!file.contents.contains("$6$"), "{}", file.path);
        assert!(!file.contents.contains("correct horse"), "{}", file.path);
        assert!(!file.contents.contains("another long"), "{}", file.path);
    }
}

#[test]
fn flake_takes_nixpkgs_by_channel() {
    for (channel, path, source) in [
        (Channel::Unstable, "inputs.nixpkgs.follows", "rikka/nixpkgs"),
        (
            Channel::Stable,
            "inputs.nixpkgs.url",
            "github:NixOS/nixpkgs/nixos-26.05",
        ),
    ] {
        let files = generated(&Answers {
            channel,
            release: "26.05".to_owned(),
            ..answers()
        });
        let flake = Document::parse(file(&files, "flake.nix")).expect("output should parse as Nix");
        assert_eq!(
            flake.get::<String>("inputs.rikka.url"),
            Ok(Some(PACKAGES_FLAKE.to_owned()))
        );
        assert_eq!(
            flake.get::<String>(path),
            Ok(Some(source.to_owned())),
            "{channel:?}"
        );
        assert!(
            file(&files, "flake.nix").contains("rikka.nixosModules.default"),
            "{channel:?}"
        );
    }
}

#[test]
fn apps_module_enables_chosen_apps() {
    let mut chosen = answers();
    chosen.choose_app("paint", false);
    chosen.choose_app("modeler", true);
    chosen.choose_app("files", false);
    let files = generated(&chosen);
    let apps = Document::parse(file(&files, "rikka/apps.nix")).expect("output should parse as Nix");
    for (id, enabled) in [
        ("browser", Some(true)),
        ("modeler", Some(true)),
        ("paint", None),
        ("files", None),
        ("base", None),
    ] {
        assert_eq!(
            apps.get::<bool>(format!("rikka.apps.{id}.enable")),
            Ok(enabled),
            "{id}"
        );
    }
    let root =
        Document::parse(file(&files, "rikka/default.nix")).expect("output should parse as Nix");
    let imports = root
        .get::<Expr>("imports")
        .expect("it reads")
        .expect("it is there");
    assert!(matches!(imports, Expr::List(paths) if paths.contains(&Expr::path("./apps.nix"))));
}

#[test]
fn nvidia_driver_matches_generation() {
    let cases = [
        (
            vec![gpu(GpuVendor::Nvidia, 0x2882, "nouveau")],
            Some(("true", "latest")),
        ),
        (
            vec![gpu(GpuVendor::Nvidia, 0x1B80, "nouveau")],
            Some(("false", "legacy_580")),
        ),
        (
            vec![
                gpu(GpuVendor::Nvidia, 0x2882, "nouveau"),
                gpu(GpuVendor::Nvidia, 0x1B80, "nouveau"),
            ],
            Some(("false", "legacy_580")),
        ),
        (vec![gpu(GpuVendor::Nvidia, 0x1180, "nouveau")], None),
        (
            vec![
                gpu(GpuVendor::Nvidia, 0x2882, "nouveau"),
                gpu(GpuVendor::Nvidia, 0x0FC6, "nouveau"),
            ],
            None,
        ),
    ];
    for (gpus, expected) in cases {
        let files = generated(&Answers {
            allow_unfree: true,
            nvidia: true,
            gpus,
            ..answers()
        });
        let desktop = file(&files, "rikka/desktop.nix");
        let Some((open, package)) = expected else {
            assert!(!desktop.contains("hardware.nvidia"), "{desktop}");
            continue;
        };
        assert!(
            desktop.contains(&format!("hardware.nvidia.open = {open};")),
            "{desktop}"
        );
        assert!(
            desktop.contains(&format!(
                "hardware.nvidia.package = config.boot.kernelPackages.nvidiaPackages.{package};"
            )),
            "{desktop}"
        );
    }
}

#[test]
fn boots_chosen_kernel() {
    let cases = [
        (Kernel::Default, None),
        (Kernel::Latest, Some("pkgs.linuxPackages_latest")),
        (Kernel::Zen, Some("pkgs.linuxPackages_zen")),
    ];
    for (kernel, expected) in cases {
        let files = generated(&Answers {
            kernel,
            ..answers()
        });
        let boot =
            Document::parse(file(&files, "rikka/boot.nix")).expect("output should parse as Nix");
        assert_eq!(
            boot.get::<AttrPath>("boot.kernelPackages"),
            Ok(expected.map(|path| AttrPath::parse(path).expect("path"))),
            "{kernel:?}"
        );
    }
}

#[test]
fn desktop_starts_rikka_as_user() {
    let files = generated(&answers());
    let desktop =
        Document::parse(file(&files, "rikka/desktop.nix")).expect("output should parse as Nix");
    assert_eq!(desktop.get::<bool>("rikka.enable"), Ok(Some(true)));
    assert_eq!(
        desktop.get::<String>("rikka.autologin.user"),
        Ok(Some("eldodebug".to_owned()))
    );
}
