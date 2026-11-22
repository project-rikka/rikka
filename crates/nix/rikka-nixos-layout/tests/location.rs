use std::mem::discriminant;
use std::path::{Path, PathBuf};

use rikka_nixos_layout::{Error, Location, Module};

fn adopt(flake: &Path, modules: &Path) {
    std::fs::create_dir_all(modules).unwrap();
    std::fs::write(flake.join("flake.nix"), "{ outputs = _: { }; }\n").unwrap();
    std::fs::write(modules.join("default.nix"), "{ imports = [ ]; }\n").unwrap();
}

fn declare(folder: &Path, flake: &str, modules: &str) -> PathBuf {
    let configuration = folder.join("configuration.json");
    let json = format!(r#"{{ "flake": "{flake}", "modules": "{modules}" }}"#);
    std::fs::write(&configuration, json).unwrap();
    configuration
}

#[test]
fn reads_adopted_location() {
    let folder = tempfile::tempdir().unwrap();
    let flake = folder.path().join("nixos");
    let modules = flake.join("hosts/laptop/rikka");
    adopt(&flake, &modules);
    let configuration = declare(
        folder.path(),
        flake.to_str().unwrap(),
        modules.to_str().unwrap(),
    );

    let location = Location::read(configuration).expect("it reads");

    assert_eq!(location.flake(), flake);
    assert_eq!(location.modules(), modules);
    assert_eq!(location.module(Module::Apps), modules.join("apps.nix"));
}

#[test]
fn refuses_locations_rikka_cannot_manage() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().to_str().unwrap().to_owned();
    let flake = folder.path().join("nixos");
    adopt(&flake, &flake.join("rikka"));
    std::fs::create_dir_all(folder.path().join("bare/rikka")).unwrap();
    std::fs::write(folder.path().join("bare/flake.nix"), "{ }\n").unwrap();

    let outside = || Error::Outside {
        modules: PathBuf::new(),
        flake: PathBuf::new(),
    };
    let cases = [
        ("nixos", "nixos/rikka", Error::NotAbsolute(PathBuf::new())),
        (
            "/nixos/../etc",
            "/nixos/../etc/rikka",
            Error::NotAbsolute(PathBuf::new()),
        ),
        ("{root}/nixos", "{root}/elsewhere/rikka", outside()),
        ("{root}/nixos", "{root}/nixos", outside()),
        (
            "{root}/missing",
            "{root}/missing/rikka",
            Error::NoFlake(PathBuf::new()),
        ),
        (
            "{root}/bare",
            "{root}/bare/rikka",
            Error::NotAdopted(PathBuf::new()),
        ),
    ];
    for (flake, modules, expected) in cases {
        let configuration = declare(
            folder.path(),
            &flake.replace("{root}", &root),
            &modules.replace("{root}", &root),
        );
        match Location::read(&configuration) {
            Err(error) if discriminant(&error) == discriminant(&expected) => {}
            other => panic!("{flake} with {modules}: {other:?}"),
        }
    }
}

#[test]
fn missing_or_malformed_configuration_is_reported() {
    let folder = tempfile::tempdir().unwrap();
    let configuration = folder.path().join("configuration.json");

    assert!(matches!(
        Location::read(&configuration),
        Err(Error::Unmanaged(path)) if path == configuration
    ));

    std::fs::write(&configuration, r#"{ "flake": "/etc/nixos" }"#).unwrap();
    assert!(matches!(
        Location::read(&configuration),
        Err(Error::Malformed { .. })
    ));
}
