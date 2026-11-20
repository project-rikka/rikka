use rikka_nix_edit::{AttrPath, Document, Error, Expr, Formals, Set};
use rikka_test_nixfmt::assert_formatted;

fn module(body: &str) -> Document {
    Document::parse(format!("{{ pkgs, ... }}:\n{{\n{body}}}\n")).expect("it is Nix")
}

fn path(text: &str) -> AttrPath {
    AttrPath::parse(text).expect("a path")
}

fn packages(names: &[&str]) -> Expr {
    Expr::with(
        Expr::reference("pkgs").unwrap(),
        Expr::list(names.iter().map(|name| Expr::reference(*name).unwrap())),
    )
}

#[test]
fn reads_value_in_any_spelling() {
    for body in [
        "  i18n.defaultLocale = \"ja_JP.UTF-8\";\n",
        "  i18n = {\n    defaultLocale = \"ja_JP.UTF-8\";\n  };\n",
        "  i18n = ({ defaultLocale = (\"ja_JP.UTF-8\"); });\n",
    ] {
        let document = module(body);
        assert_eq!(
            document.get::<String>("i18n.defaultLocale"),
            Ok(Some("ja_JP.UTF-8".to_owned())),
            "{body}"
        );
    }
    assert_eq!(module("").get::<String>("i18n.defaultLocale"), Ok(None));
}

#[test]
fn flat_attributes_read_as_set() {
    let document = module(
        "  services.openssh.enable = true;\n  services.openssh.settings = {\n    PermitRootLogin = \"no\";\n  };\n",
    );
    let openssh = document
        .get::<Set>("services.openssh")
        .unwrap()
        .expect("it is there");
    assert_eq!(openssh.get(&path("enable")), Some(&Expr::Bool(true)));
    assert_eq!(
        openssh.get(&path("settings.PermitRootLogin")),
        Some(&Expr::from("no"))
    );
}

#[test]
fn replace_preserves_surroundings() {
    let mut document = module(
        "  # mine\n  environment.systemPackages = with pkgs; [\n    firefox\n  ]; # kept\n  programs.git.enable = true;\n",
    );
    document
        .set(
            "environment.systemPackages",
            packages(&["firefox", "ripgrep"]),
        )
        .unwrap();
    assert_eq!(
        document.source(),
        "{ pkgs, ... }:\n{\n  # mine\n  environment.systemPackages = with pkgs; [\n    firefox\n    ripgrep\n  ]; # kept\n  programs.git.enable = true;\n}\n"
    );
}

#[test]
fn insert_joins_sibling_set() {
    let mut document = module("  i18n = {\n    defaultLocale = \"ja_JP.UTF-8\";\n  };\n  a = 1;\n");
    document
        .set("i18n.extraLocaleSettings.LC_MESSAGES", "en_US.UTF-8")
        .unwrap();
    document.set("time.timeZone", "Asia/Tokyo").unwrap();
    assert_eq!(
        document.source(),
        "{ pkgs, ... }:\n{\n  i18n = {\n    defaultLocale = \"ja_JP.UTF-8\";\n    extraLocaleSettings.LC_MESSAGES = \"en_US.UTF-8\";\n  };\n  a = 1;\n  time.timeZone = \"Asia/Tokyo\";\n}\n"
    );
}

#[test]
fn set_replaces_nested_attributes() {
    let mut document =
        module("  i18n.defaultLocale = \"C\";\n  i18n.supportedLocales = [ ];\n  a = 1;\n");
    document
        .set("i18n", Set::new().bind("defaultLocale", "ja_JP.UTF-8"))
        .unwrap();
    assert_eq!(
        document.source(),
        "{ pkgs, ... }:\n{\n  a = 1;\n  i18n = {\n    defaultLocale = \"ja_JP.UTF-8\";\n  };\n}\n"
    );
}

#[test]
fn remove_drops_empty_set_keeps_comments() {
    let mut document =
        module("  # mine\n  a = 1;\n  services.geoclue2 = {\n    enable = true;\n  };\n  b = 2;\n");
    document.remove("services.geoclue2.enable").unwrap();
    document.remove("a").unwrap();
    assert_eq!(
        document.source(),
        "{ pkgs, ... }:\n{\n  # mine\n  b = 2;\n}\n"
    );
    let before = document.clone();
    document.remove("services.openssh.enable").unwrap();
    assert_eq!(document, before);
}

#[test]
fn remove_shared_line_bindings() {
    let mut document = module("  a.b = 1;   a.c = 2;\n  x = 3;\n");
    document.remove("a").unwrap();
    assert_eq!(document.source(), "{ pkgs, ... }:\n{\n  x = 3;\n}\n");
}

#[test]
fn set_then_remove_restores_blank_module() {
    let blank = Document::parse("{ ... }:\n{ }\n").unwrap();
    let mut document = blank.clone();
    document.set("nix.gc.automatic", true).unwrap();
    document.set("nix.gc.dates", "weekly").unwrap();
    document.remove("nix.gc.automatic").unwrap();
    document.remove("nix.gc.dates").unwrap();
    assert_eq!(document, blank);
}

#[test]
fn include_keeps_existing_items() {
    let mut document =
        module("  networking.firewall.allowedUDPPorts = [\n    47810\n    47811\n  ];\n");
    document
        .include("networking.firewall.allowedUDPPorts", 47900)
        .unwrap();
    let once = document.clone();
    document
        .include("networking.firewall.allowedUDPPorts", 47900)
        .unwrap();
    assert_eq!(document, once);
    assert_eq!(
        document.get::<Vec<i64>>("networking.firewall.allowedUDPPorts"),
        Ok(Some(vec![47810, 47811, 47900]))
    );
    document
        .include(
            "networking.firewall.interfaces.tailscale0.allowedUDPPorts",
            47900,
        )
        .unwrap();
    assert_eq!(
        document.get::<Vec<i64>>("networking.firewall.interfaces.tailscale0.allowedUDPPorts"),
        Ok(Some(vec![47900]))
    );
    assert_formatted(document.source());
}

#[test]
fn exclude_removes_only_item() {
    let mut document = module("  ports = [\n    47810\n    47900\n  ];\n  mine = [ 47900 ];\n");
    document.exclude("ports", 47900).unwrap();
    assert_eq!(document.get::<Vec<i64>>("ports"), Ok(Some(vec![47810])));
    document.exclude("mine", 47900).unwrap();
    assert_eq!(document.get::<Expr>("mine"), Ok(None));
    let before = document.clone();
    document.exclude("ports", 1).unwrap();
    document.exclude("absent", 1).unwrap();
    assert_eq!(document, before);
}

#[test]
fn membership_edits_refuse_unreadable_lists() {
    let mut document = module("  ports = [ 1 ] ++ extra;\n  packages = with pkgs; [ git ];\n");
    let before = document.clone();
    assert!(matches!(
        document.include("ports", 2),
        Err(Error::Opaque { .. })
    ));
    assert!(matches!(
        document.exclude("ports", 1),
        Err(Error::Opaque { .. })
    ));
    assert!(matches!(
        document.include("packages", 2),
        Err(Error::Mismatch { .. })
    ));
    assert_eq!(document, before);
}

#[test]
fn rejects_values_requiring_evaluation() {
    let mut document = module("  services = lib.mkIf true { };\n  inherit (pkgs) hello;\n");
    let before = document.clone();
    assert_eq!(
        document.set("services.openssh.enable", true),
        Err(Error::Opaque {
            path: path("services"),
            found: "a function application"
        })
    );
    assert!(matches!(
        document.get::<Expr>("hello"),
        Err(Error::Opaque {
            found: "an inherited name",
            ..
        })
    ));
    assert!(matches!(
        document.set("a", Expr::path("./a b")),
        Err(Error::Path(_))
    ));
    assert_eq!(document, before);
}

#[test]
fn strings_are_escaped() {
    let mut document = module("");
    let hostile = r#"a"; imports = [ ./evil.nix ]; x = ""#;
    document.set("networking.hostName", hostile).unwrap();
    document
        .set("users.motd", "${builtins.readFile /etc/shadow}\\")
        .unwrap();
    assert!(
        document
            .source()
            .contains(r#""\${builtins.readFile /etc/shadow}\\""#)
    );
    assert_eq!(
        document.paths(),
        Ok(vec![path("networking.hostName"), path("users.motd")])
    );
    assert_eq!(
        document.get::<String>("networking.hostName"),
        Ok(Some(hostile.to_owned()))
    );
    assert_eq!(
        document.get::<String>("users.motd"),
        Ok(Some("${builtins.readFile /etc/shadow}\\".to_owned()))
    );
    assert!(document.set("a = 1; b", 1).is_err());
}

#[test]
fn rejects_unrepresentable_values() {
    let mut document = module("");
    let before = document.clone();
    let hidden = Expr::list([Expr::from(Set::new().bind("a", f64::NAN))]);
    assert!(matches!(document.set("a", hidden), Err(Error::Number(_))));
    assert!(matches!(
        document.set("a", Expr::Int(i64::MIN)),
        Err(Error::Number(_))
    ));
    assert_eq!(
        document.set("a", Expr::lambda(Formals::new(["a b"]), Set::new())),
        Err(Error::Identifier("a b".to_owned()))
    );
    assert_eq!(
        document.set("a", Set::new().inherit(["rec"])),
        Err(Error::Identifier("rec".to_owned()))
    );
    assert_eq!(document, before);
}

#[test]
fn add_formal_extends_the_argument_pattern() {
    for (source, written) in [
        ("{ ... }:\n{ }\n", "{ lib, ... }:\n{ }\n"),
        (
            "# Head.\n{ config, pkgs, ... }:\n{\n  a = 1;\n}\n",
            "# Head.\n{\n  config,\n  pkgs,\n  lib,\n  ...\n}:\n{\n  a = 1;\n}\n",
        ),
        ("{ lib, ... }:\n{ }\n", "{ lib, ... }:\n{ }\n"),
    ] {
        let mut document = Document::parse(source).unwrap();
        document.add_formal("lib").unwrap();
        assert_eq!(document.source(), written, "{source}");
        assert_formatted(document.source());
    }
    for (source, refusal) in [
        ("{ }\n", Error::NotFunction),
        ("args: { }\n", Error::NotFunction),
        (
            "{ pkgs ? let a = 1; in a, ... }:\n{ }\n",
            Error::OpaquePattern("a let expression"),
        ),
        (
            "{\n  pkgs, # kept\n  ...\n}:\n{ }\n",
            Error::OpaquePattern("a comment"),
        ),
    ] {
        let mut document = Document::parse(source).unwrap();
        assert_eq!(document.add_formal("lib"), Err(refusal), "{source}");
        assert_eq!(document.source(), source);
    }
}

#[test]
fn syntax_error_reports_location() {
    assert!(matches!(
        Document::parse("{\n  a = ;\n}\n"),
        Err(Error::Syntax { line: 2, .. })
    ));
    assert_eq!(
        Document::parse("[ ]\n").unwrap().get::<bool>("a"),
        Err(Error::NotAttrSet)
    );
}

#[test]
fn written_documents_match_nixfmt() {
    let flake = Document::with_header(
        "A flake.",
        Set::new()
            .bind("description", "NixOS configuration for rikka")
            .blank()
            .bind(
                "inputs",
                Set::new()
                    .bind("nixpkgs.url", "github:NixOS/nixpkgs/nixos-unstable")
                    .bind("rikka.inputs.nixpkgs.follows", "nixpkgs"),
            )
            .blank()
            .bind(
                "outputs",
                Expr::lambda(
                    Formals::new(["nixpkgs", "rikka"])
                        .ellipsis()
                        .bound("inputs"),
                    Expr::let_in(
                        Set::new().bind(
                            "system",
                            Expr::apply(
                                Expr::reference("nixpkgs.lib.nixosSystem").unwrap(),
                                Set::new()
                                    .bind("specialArgs", Set::new().inherit(["inputs"]))
                                    .bind(
                                        "modules",
                                        [
                                            Expr::reference("rikka.nixosModules.rikka").unwrap(),
                                            Expr::path("./configuration.nix"),
                                        ],
                                    ),
                            ),
                        ),
                        Set::new().bind(
                            "nixosConfigurations",
                            Set::new().bind_dynamic(
                                Expr::reference("system.config.networking.hostName").unwrap(),
                                Expr::reference("system").unwrap(),
                            ),
                        ),
                    ),
                ),
            ),
    )
    .unwrap();
    assert_formatted(flake.source());

    let configuration = Document::new(Expr::lambda(
        Formals::new(["config", "pkgs"]).ellipsis(),
        Set::new()
            .bind(
                "imports",
                [
                    Expr::path("./hardware-configuration.nix"),
                    Expr::path("./rikka"),
                ],
            )
            .blank()
            .comment("--- Users ---")
            .bind(
                ["users", "users", "someone else"],
                Set::new()
                    .bind("isNormalUser", true)
                    .bind("extraGroups", ["wheel", "networkmanager"]),
            )
            .bind(
                ["fileSystems", "/", "options"],
                ["compress=zstd", "noatime"],
            )
            .bind("zramSwap.memoryPercent", 50)
            .bind("hardware.nvidia.open", false)
            .bind("boot.initrd.kernelModules", ["i915"])
            .bind("environment.systemPackages", packages(&["git"]))
            .bind("environment.etc", Set::new())
            .bind("hardware.graphics.extraPackages", packages(&[]))
            .bind("a.float", -1.5)
            .bind(
                "a.list",
                [Expr::Int(-1), Expr::Null, Expr::list(["x", "y"])],
            )
            .bind(
                "hardware.graphics.enable32Bit",
                Expr::reference("pkgs.stdenv.hostPlatform.isx86_64").unwrap(),
            ),
    ))
    .unwrap();
    assert_formatted(configuration.source());
}

#[test]
fn edits_preserve_formatting() {
    let mut document = Document::new(Expr::lambda(
        Formals::new(["pkgs"]).ellipsis(),
        Set::new()
            .comment("--- Graphics ---")
            .bind("hardware.graphics.enable32Bit", true)
            .blank()
            .comment("--- Packages ---")
            .bind("environment.systemPackages", packages(&["git"]))
            .bind("services.geoclue2", Set::new().bind("enable", true)),
    ))
    .unwrap();
    assert_formatted(document.source());

    document
        .set("environment.systemPackages", packages(&["git", "ripgrep"]))
        .unwrap();
    document
        .set(
            "services.geoclue2.appConfig.\"rikka-shell\".isAllowed",
            true,
        )
        .unwrap();
    document
        .set(
            "nix.settings.experimental-features",
            ["nix-command", "flakes"],
        )
        .unwrap();
    document.set("environment.etc", Set::new()).unwrap();
    document.set("environment.etc.foo.text", "bar").unwrap();
    assert_formatted(document.source());

    document.remove("hardware.graphics.enable32Bit").unwrap();
    document.remove("services.geoclue2").unwrap();
    document.remove("environment.etc.foo.text").unwrap();
    assert_formatted(document.source());
    assert_eq!(
        document.paths().unwrap(),
        [
            path("environment.systemPackages"),
            path("nix.settings.experimental-features"),
        ]
    );
}
