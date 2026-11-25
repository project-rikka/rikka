use launcher_meta::{Arch, Environment, Error, Os, Platform, Substitutions, Version, expand};

const SHA: &str = "1227f9e0666314f9de41477e3ec277e542ed7f7b";

fn game() -> String {
    format!(
        r#"{{
            "id": "26.3",
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {{"game": ["--version", "${{version_name}}"], "jvm": ["-cp", "${{classpath}}"]}},
            "assetIndex": {{"id": "34", "sha1": "{SHA}", "size": 1, "totalSize": 2, "url": "https://example.com/34.json"}},
            "downloads": {{"client": {{"sha1": "{SHA}", "size": 3, "url": "https://example.com/client.jar"}}}},
            "javaVersion": {{"component": "java-runtime-epsilon", "majorVersion": 25}},
            "libraries": [
                {{"name": "org.ow2.asm:asm:9.0",
                  "downloads": {{"artifact": {{"path": "org/ow2/asm/asm/9.0/asm-9.0.jar", "sha1": "{SHA}", "size": 4, "url": "https://libraries.minecraft.net/org/ow2/asm/asm/9.0/asm-9.0.jar"}}}}}},
                {{"name": "org.lwjgl:lwjgl:3.3.3:natives-linux",
                  "downloads": {{"artifact": {{"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar", "sha1": "{SHA}", "size": 5, "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"}}}},
                  "rules": [{{"action": "allow", "os": {{"name": "linux"}}}}]}},
                {{"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
                  "downloads": {{"artifact": {{"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar", "sha1": "{SHA}", "size": 6, "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"}}}},
                  "rules": [{{"action": "allow", "os": {{"name": "windows"}}}}]}}
            ]
        }}"#
    )
}

fn loader() -> String {
    format!(
        r#"{{
            "id": "fabric-loader-0.20.1-26.3",
            "inheritsFrom": "26.3",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {{"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]}},
            "libraries": [
                {{"name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/", "sha1": "{SHA}", "size": 7}},
                {{"name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/"}}
            ]
        }}"#
    )
}

fn merged() -> Version {
    let parent = Version::from_json(game().as_bytes()).expect("game parses");
    let child = Version::from_json(loader().as_bytes()).expect("loader parses");
    child.inherit(parent).expect("loader inherits the game")
}

#[test]
fn loader_inherits_game() {
    let client = merged().into_client().expect("complete version");
    assert_eq!(client.id, "fabric-loader-0.20.1-26.3");
    assert_eq!(
        client.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert_eq!(client.asset_index.id, "34");
    assert_eq!(client.java.major_version, 25);
    let environment = Environment::new(Platform {
        os: Os::Linux,
        arch: Arch::X86_64,
    });
    let jvm = expand(
        &client.arguments.jvm,
        &environment,
        &Substitutions::new().with("classpath", "a:b"),
    )
    .expect("expands");
    assert_eq!(
        jvm,
        [
            "-cp",
            "a:b",
            "-DFabricMcEmu= net.minecraft.client.main.Main "
        ]
    );
}

#[test]
fn child_library_replaces_parent_version() {
    let client = merged().into_client().expect("complete version");
    let linux = Environment::new(Platform {
        os: Os::Linux,
        arch: Arch::X86_64,
    });
    let paths: Vec<_> = client
        .artifacts(&linux)
        .expect("artifacts resolve")
        .into_iter()
        .map(|artifact| (artifact.path.to_string(), artifact.url))
        .collect();
    assert_eq!(
        paths,
        [
            (
                "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".to_owned(),
                "https://maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".to_owned()
            ),
            (
                "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar".to_owned(),
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
                    .to_owned()
            ),
            (
                "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar".to_owned(),
                "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn wrong_parent_is_refused() {
    let mut game = Version::from_json(game().as_bytes()).expect("game parses");
    game.id = "26.2".to_owned();
    let child = Version::from_json(loader().as_bytes()).expect("loader parses");
    assert!(matches!(child.inherit(game), Err(Error::Parent { .. })));
}

#[test]
fn traversing_paths_are_refused() {
    for path in [
        "../evil.jar",
        "/etc/passwd",
        "a/../../b.jar",
        "C:/x.jar",
        "a\\b.jar",
    ] {
        let json = format!(
            r#"{{"name": "a:b:1", "downloads": {{"artifact": {{"path": "{}", "sha1": "{SHA}", "size": 1, "url": "https://x/y"}}}}}}"#,
            path.replace('\\', "\\\\")
        );
        assert!(
            serde_json::from_str::<launcher_meta::Library>(&json).is_err(),
            "{path}"
        );
    }
}
