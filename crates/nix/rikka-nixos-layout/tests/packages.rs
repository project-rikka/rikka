use rikka_nix_edit::Document;
use rikka_nixos_layout::Packages;

fn read(body: &str) -> Vec<String> {
    Document::parse(format!(
        "{{ pkgs, ... }}:\n{{\n  environment.systemPackages = {body};\n}}\n"
    ))
    .unwrap()
    .get::<Packages>("environment.systemPackages")
    .unwrap()
    .expect("it is there")
    .names()
    .collect()
}

#[test]
fn package_list_reads_through_scope() {
    assert_eq!(
        read("with pkgs; [ git qt6Packages.fcitx5-mozc ]"),
        ["git", "qt6Packages.fcitx5-mozc"]
    );
    assert_eq!(read("[ pkgs.git pkgs.ripgrep ]"), ["git", "ripgrep"]);
    assert_eq!(
        read("with pkgs; [ pkgs.\"2048-in-terminal\" ]"),
        ["2048-in-terminal"]
    );
    let opaque = Document::parse("{\n  a = with pkgs; [ (f x) ];\n}\n").unwrap();
    assert!(opaque.get::<Packages>("a").is_err());
}

#[test]
fn package_list_round_trips() {
    let packages = Packages::new(["git", "qt6Packages.fcitx5-mozc", "2048-in-terminal"]);
    let mut document = Document::parse("{ pkgs, ... }:\n{ }\n").unwrap();
    document
        .set("environment.systemPackages", packages.clone())
        .unwrap();
    assert!(
        document.source().contains(
            "  environment.systemPackages = with pkgs; [\n    git\n    qt6Packages.fcitx5-mozc\n    pkgs.\"2048-in-terminal\"\n  ];\n"
        ),
        "{document}"
    );
    assert_eq!(
        document.get::<Packages>("environment.systemPackages"),
        Ok(Some(packages))
    );
}

#[test]
fn package_name_is_not_parsed_as_nix() {
    let mut document = Document::parse("{ pkgs, ... }:\n{ }\n").unwrap();
    document
        .set("a", Packages::new(["git ]; x = [ y"]))
        .unwrap();
    assert_eq!(document.paths().unwrap().len(), 1, "{document}");
    assert_eq!(
        document
            .get::<Packages>("a")
            .unwrap()
            .expect("it is there")
            .names()
            .collect::<Vec<_>>(),
        ["git ]; x = [ y"]
    );
}
