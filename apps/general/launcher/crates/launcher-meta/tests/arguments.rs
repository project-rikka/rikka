use launcher_meta::{
    Arch, Argument, Environment, Error, Feature, Os, Platform, Substitutions, expand,
};

const LINUX: Platform = Platform {
    os: Os::Linux,
    arch: Arch::X86_64,
};

fn arguments() -> Vec<Argument> {
    serde_json::from_str(
        r#"[
            "--username", "${auth_player_name}",
            "--gameDir", "${game_directory}",
            {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
             "value":["--width","${resolution_width}","--height","${resolution_height}"]},
            {"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],
             "value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]},
            {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"}
        ]"#,
    )
    .expect("arguments parse")
}

fn values() -> Substitutions {
    Substitutions::new()
        .with("auth_player_name", "Steve")
        .with("game_directory", "/games/My World")
        .with("resolution_width", "1280")
        .with("resolution_height", "720")
}

#[test]
fn expansion_keeps_only_allowed_arguments() {
    let expanded = expand(&arguments(), &Environment::new(LINUX), &values()).expect("expands");
    assert_eq!(
        expanded,
        ["--username", "Steve", "--gameDir", "/games/My World"]
    );
}

#[test]
fn expansion_follows_features() {
    let environment = Environment::new(LINUX).with(Feature::CustomResolution);
    let expanded = expand(&arguments(), &environment, &values()).expect("expands");
    assert_eq!(&expanded[4..], ["--width", "1280", "--height", "720"]);
}

#[test]
fn unknown_placeholder_fails() {
    let environment = Environment::new(LINUX).with(Feature::QuickPlaySingleplayer);
    let error = expand(&arguments(), &environment, &values()).expect_err("the world is unset");
    assert!(matches!(
        error,
        Error::Placeholder { placeholder, .. } if placeholder == "quickPlaySingleplayer"
    ));
}
