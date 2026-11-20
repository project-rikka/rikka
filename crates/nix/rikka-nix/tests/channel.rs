use rikka_nix::Channel;

#[test]
fn flake_reports_nixpkgs_input() {
    for (inputs, expected) in [
        (
            r#"nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";"#,
            Some(Channel::Release("26.05".to_owned())),
        ),
        (
            r#"nixpkgs.follows = "rikka/nixpkgs";"#,
            Some(Channel::Unstable),
        ),
        (r#"nixpkgs.follows = "other/nixpkgs";"#, None),
    ] {
        let flake = format!(
            r#"
{{
  inputs = {{
    rikka.url = "github:project-rikka/packages";
    {inputs}
  }};

  outputs = inputs@{{ nixpkgs, rikka, ... }}: {{ }};
}}
"#
        );
        assert_eq!(Channel::from_flake(&flake), expected, "{inputs}");
    }
}

#[test]
fn channel_from_url_or_running_version() {
    for url in [
        "github:NixOS/nixpkgs/nixos-unstable",
        "nixpkgs/nixos-unstable",
    ] {
        assert_eq!(Channel::from_url(url), Some(Channel::Unstable), "{url}");
    }
    assert_eq!(Channel::from_url("github:NixOS/nixpkgs"), None);
    assert_eq!(Channel::from_url("path:/home/eldodebug/nixpkgs"), None);

    assert_eq!(
        Channel::from_version("26.05.20260830.abcdef\n"),
        Some(Channel::Release("26.05".to_owned()))
    );
    assert_eq!(
        Channel::from_version("26.11pre-git"),
        Some(Channel::Unstable)
    );
    assert_eq!(Channel::from_version("not a version"), None);
}
