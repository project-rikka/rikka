use nixpkgs_search::{Cancel, Error, License, Nixpkgs};

use crate::support::{LOOKUP_JSON, Nix};

const SYSTEM: &str = "x86_64-linux";

fn nixpkgs() -> Nixpkgs {
    Nixpkgs::open(SYSTEM, &Cancel::new()).expect("failed to open nixpkgs")
}

#[test]
fn lookup_reads_every_field() {
    let nix = Nix::with_stdout(LOOKUP_JSON);

    let packages = nixpkgs()
        .lookup(&["ripgrep", "python3Packages.httpx"], &Cancel::new())
        .expect("lookup failed");

    assert!(
        nix.arguments()
            .contains("nixpkgs#legacyPackages.x86_64-linux")
    );
    let package = |attribute: &str| {
        packages
            .iter()
            .find(|packages| packages.attribute == attribute)
            .unwrap_or_else(|| panic!("{attribute} missing from lookup"))
    };
    let ripgrep = package("ripgrep");
    assert_eq!(ripgrep.name, "ripgrep");
    assert_eq!(ripgrep.version, "15.2.0");
    assert_eq!(ripgrep.set, None);
    assert_eq!(ripgrep.main_program.as_deref(), Some("rg"));
    assert_eq!(ripgrep.outputs, ["out"]);
    assert!(ripgrep.platforms.contains(&SYSTEM.to_owned()));
    assert_eq!(ripgrep.homepages, ["https://github.com/BurntSushi/ripgrep"]);
    assert!(ripgrep.maintainers.iter().any(|who| who.label().is_some()));
    assert_eq!(
        ripgrep
            .licenses
            .iter()
            .map(License::label)
            .collect::<Vec<&str>>(),
        ["Unlicense", "MIT"]
    );
    assert_eq!(
        ripgrep.position.as_deref(),
        Some("pkgs/by-name/ri/ripgrep/package.nix:66")
    );
    let httpx = package("python3Packages.httpx");
    assert_eq!(httpx.set.as_deref(), Some("python3Packages"));
    assert_eq!(httpx.name, "httpx");
}

#[test]
fn lookup_escapes_attribute_strings() {
    let nix = Nix::with_stdout(b"[]");

    let packages = nixpkgs()
        .lookup(
            &[
                r#"a" ++ import "/etc/shadow"#,
                "${pkgs.hello}",
                "two\nlines",
            ],
            &Cancel::new(),
        )
        .expect("lookup failed");

    assert!(packages.is_empty());
    let arguments = nix.arguments();
    assert!(
        arguments.contains(r#"[ "a\" ++ import \"/etc/shadow" "\${pkgs.hello}" "two\nlines" ])"#),
        "{arguments}"
    );
}

#[test]
fn failure_reports_innermost_nix_error() {
    let opened = {
        let _nix = Nix::with_stdout(b"x86_64-linux");
        nixpkgs()
    };
    let _nix = Nix::failing(
        "error:\n       … while calling the 'throw' builtin\n         at «string»:1:7:\n\n       error: no such attribute\n"
            .as_bytes(),
    );

    let result = opened.lookup(&["missing"], &Cancel::new());

    assert!(
        matches!(&result, Err(Error::Failed(message)) if message == "no such attribute"),
        "{result:?}"
    );
}

#[test]
fn invalid_output_is_error() {
    let _nix = Nix::with_stdout(b"error: attribute missing");

    let read = nixpkgs().lookup(&["ripgrep"], &Cancel::new());

    assert!(matches!(read, Err(Error::Json(_))), "{read:?}");
}
