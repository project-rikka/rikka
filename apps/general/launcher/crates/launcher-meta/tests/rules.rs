use launcher_meta::{Arch, Environment, Feature, Os, Platform, Rule, allowed};

fn rules(json: &str) -> Vec<Rule> {
    serde_json::from_str(json).expect("rules parse")
}

const LINUX: Platform = Platform {
    os: Os::Linux,
    arch: Arch::X86_64,
};

const WINDOWS: Platform = Platform {
    os: Os::Windows,
    arch: Arch::X86_64,
};

#[test]
fn rules_follow_last_matching_action() {
    let cases = [
        ("[]", Environment::new(LINUX), true),
        (
            r#"[{"action":"allow","os":{"name":"osx"}}]"#,
            Environment::new(LINUX),
            false,
        ),
        (
            r#"[{"action":"allow","os":{"name":"windows"}}]"#,
            Environment::new(WINDOWS),
            true,
        ),
        (
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"linux"}}]"#,
            Environment::new(LINUX),
            false,
        ),
        (
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"linux"}}]"#,
            Environment::new(WINDOWS),
            true,
        ),
        (
            r#"[{"action":"allow","os":{"arch":"x86"}}]"#,
            Environment::new(LINUX),
            false,
        ),
        (
            r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#,
            Environment::new(LINUX),
            false,
        ),
        (
            r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#,
            Environment::new(LINUX).with(Feature::CustomResolution),
            true,
        ),
        (
            r#"[{"action":"allow","features":{"is_demo_user":true}}]"#,
            Environment::new(LINUX).with(Feature::CustomResolution),
            false,
        ),
    ];
    for (json, environment, expected) in cases {
        assert_eq!(allowed(&rules(json), &environment), expected, "{json}");
    }
}
