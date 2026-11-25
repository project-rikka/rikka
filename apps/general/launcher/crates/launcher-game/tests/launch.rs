use std::path::PathBuf;

use launcher_game::{Launch, Player, QuickPlay};
use launcher_install::{Installation, Java};
use launcher_meta::{Arch, GameTarget, Os, Platform, Version};

const SHA: &str = "1227f9e0666314f9de41477e3ec277e542ed7f7b";

fn installation() -> Installation {
    let json = format!(
        r#"{{
            "id": "fabric-loader-0.20.1-26.3",
            "type": "release",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {{
                "jvm": ["-Djava.library.path=${{natives_directory}}/java", "-Dminecraft.launcher.brand=${{launcher_name}}", "-cp", "${{classpath}}"],
                "game": ["--username", "${{auth_player_name}}", "--version", "${{version_name}}",
                    "--gameDir", "${{game_directory}}", "--assetsDir", "${{assets_root}}",
                    "--assetIndex", "${{assets_index_name}}", "--uuid", "${{auth_uuid}}",
                    "--accessToken", "${{auth_access_token}}", "--clientId", "${{clientid}}",
                    "--xuid", "${{auth_xuid}}", "--versionType", "${{version_type}}",
                    {{"rules": [{{"action": "allow", "features": {{"has_custom_resolution": true}}}}],
                      "value": ["--width", "${{resolution_width}}", "--height", "${{resolution_height}}"]}},
                    {{"rules": [{{"action": "allow", "features": {{"is_quick_play_multiplayer": true}}}}],
                      "value": ["--quickPlayMultiplayer", "${{quickPlayMultiplayer}}"]}},
                    {{"rules": [{{"action": "allow", "features": {{"has_quick_plays_support": true}}}}],
                      "value": ["--quickPlayPath", "${{quickPlayPath}}"]}}
                ]
            }},
            "assetIndex": {{"id": "34", "sha1": "{SHA}", "size": 1, "url": "https://example.com/34.json"}},
            "downloads": {{"client": {{"sha1": "{SHA}", "size": 3, "url": "https://example.com/client.jar"}}}},
            "javaVersion": {{"component": "java-runtime-epsilon", "majorVersion": 25}},
            "libraries": []
        }}"#
    );
    let version = Version::from_json(json.as_bytes())
        .expect("version parses")
        .into_client()
        .expect("complete version");
    Installation {
        target: GameTarget {
            minecraft: "26.3".to_owned(),
            fabric_loader: "0.20.1".to_owned(),
        },
        version,
        classpath: vec![
            PathBuf::from("/data/libraries/a.jar"),
            PathBuf::from("/data/versions/26.3/26.3.jar"),
        ],
        libraries: PathBuf::from("/data/libraries"),
        assets: PathBuf::from("/data/assets"),
        natives: PathBuf::from("/data/natives/26.3"),
        java: Java {
            executable: PathBuf::from("/java/bin/java"),
            library_path: None,
        },
    }
}

fn player() -> Player {
    Player {
        name: "Steve".to_owned(),
        id: "069a79f444e94726a5befca90e38aaf5".to_owned(),
        xuid: "2535".to_owned(),
        access_token: "token".to_owned(),
    }
}

#[test]
fn command_line_orders_jvm_main_class_and_game_arguments() {
    let launch = Launch::new(installation(), "/data/instances/survival", player())
        .memory(6144)
        .jvm_arguments(["-XX:+UseZGC".to_owned()])
        .window(1280, 720)
        .quick_play(Some(QuickPlay::Server("mc.example.com".to_owned())))
        .launcher_version("1.2.3");
    let arguments = launch
        .arguments(Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        })
        .expect("arguments build");
    assert_eq!(
        arguments,
        [
            "-Djava.library.path=/data/natives/26.3/java",
            "-Dminecraft.launcher.brand=rikka-launcher",
            "-cp",
            "/data/libraries/a.jar:/data/versions/26.3/26.3.jar",
            "-Xmx6144M",
            "-XX:+UseZGC",
            "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "--username",
            "Steve",
            "--version",
            "fabric-loader-0.20.1-26.3",
            "--gameDir",
            "/data/instances/survival",
            "--assetsDir",
            "/data/assets",
            "--assetIndex",
            "34",
            "--uuid",
            "069a79f444e94726a5befca90e38aaf5",
            "--accessToken",
            "token",
            "--clientId",
            "c4502edb-87c6-40cb-b595-64a280cf8906",
            "--xuid",
            "2535",
            "--versionType",
            "release",
            "--width",
            "1280",
            "--height",
            "720",
            "--quickPlayMultiplayer",
            "mc.example.com",
        ]
    );
}

#[test]
fn windows_classpath_uses_semicolons() {
    let arguments = Launch::new(installation(), "C:/games", player())
        .fullscreen(true)
        .arguments(Platform {
            os: Os::Windows,
            arch: Arch::X86_64,
        })
        .expect("arguments build");
    assert_eq!(
        arguments[3],
        "/data/libraries/a.jar;/data/versions/26.3/26.3.jar"
    );
    assert_eq!(arguments.last().map(String::as_str), Some("--fullscreen"));
}
