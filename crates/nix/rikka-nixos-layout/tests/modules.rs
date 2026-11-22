use rikka_nix_edit::{Document, Expr};
use rikka_nixos_layout::{Error, HEADER, Module, Modules};

#[test]
fn modules_named_by_layout() {
    for module in Module::EVERY {
        assert_eq!(Module::from_name(module.name()), Some(module));
    }
    assert_eq!(Module::from_name("default"), None);
    assert_eq!(Module::from_name("../../../etc/shadow"), None);
    assert_eq!(Module::from_name(""), None);
}

#[test]
fn blank_modules_formatted_and_imported() {
    let header: Vec<String> = HEADER.lines().map(|line| format!("# {line}")).collect();
    for module in Module::EVERY {
        let blank = module.blank();
        let first: Vec<&str> = blank.source().lines().take(2).collect();
        assert_eq!(first, header, "{module:?}");
        rikka_test_nixfmt::assert_formatted(blank.source());
    }
    let imports = Module::Root
        .blank()
        .get::<Expr>("imports")
        .expect("it reads")
        .expect("it is there");
    let expected: Vec<Expr> = Module::ALL
        .iter()
        .map(|module| Expr::path(format!("./{}", module.file_name())))
        .collect();
    assert_eq!(imports, Expr::List(expected));
}

#[test]
fn reads_disk_modules_keeps_layout_root() {
    let directory = tempfile::tempdir().expect("a directory");
    assert_eq!(
        Modules::read(directory.path()).expect("it reads"),
        Modules::blank()
    );

    let packages = "{ pkgs, ... }:\n{\n  environment.systemPackages = with pkgs; [ git ];\n}\n";
    std::fs::write(directory.path().join("default.nix"), "{ imports = [ ]; }\n").unwrap();
    std::fs::write(directory.path().join("packages.nix"), packages).unwrap();
    let mut modules = Modules::read(directory.path()).expect("it reads");
    assert_eq!(modules.get(Module::Root), &Module::Root.blank());
    assert_eq!(
        modules.get(Module::Packages),
        &Document::parse(packages).unwrap()
    );

    modules
        .get_mut(Module::Nix)
        .set("nixpkgs.config.allowUnfree", true)
        .unwrap();
    assert_eq!(
        modules
            .get(Module::Nix)
            .get::<bool>("nixpkgs.config.allowUnfree"),
        Ok(Some(true))
    );
    assert_eq!(modules.get(Module::System), &Module::System.blank());
}

#[test]
fn invalid_module_error_names_module() {
    let directory = tempfile::tempdir().expect("a directory");
    std::fs::write(directory.path().join("packages.nix"), "{ a = ; }").unwrap();
    match Modules::read(directory.path()) {
        Err(Error::Invalid { path, .. }) => assert!(path.ends_with("packages.nix")),
        other => panic!("{other:?}"),
    }
}
