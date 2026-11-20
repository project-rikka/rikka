use std::path::Path;

const RUNTIME: &str = "rikka_wayland_wire";
const SHARED: &str = "rikka_wayland_protocols";

fn scan(directory: &Path, protocol: &str) -> rikka_wayland_scanner::Result<String> {
    let input = directory.join("protocol.xml");
    std::fs::write(&input, protocol).unwrap();
    rikka_wayland_scanner::server(&[input], RUNTIME, SHARED)
}

#[test]
fn generated_code_matches_golden() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("protocol.xml");
    std::fs::write(&input, include_str!("golden/example.xml")).unwrap();
    let inputs = [input];
    for (side, generated, golden) in [
        (
            "shared",
            rikka_wayland_scanner::shared(&inputs, RUNTIME),
            include_str!("golden/shared.rs.golden"),
        ),
        (
            "server",
            rikka_wayland_scanner::server(&inputs, RUNTIME, SHARED),
            include_str!("golden/server.rs.golden"),
        ),
        (
            "client",
            rikka_wayland_scanner::client(&inputs, RUNTIME, SHARED),
            include_str!("golden/client.rs.golden"),
        ),
    ] {
        assert_eq!(generated.unwrap(), golden, "{side}");
    }
}

#[test]
fn rejects_unknown_references() {
    for (protocol, named) in [
        (
            r#"<protocol name="broken">
                 <interface name="ex_broken" version="1">
                   <request name="use"><arg name="thing" type="object" interface="ex_missing"/></request>
                 </interface>
               </protocol>"#,
            "unknown interface ex_missing",
        ),
        (
            r#"<protocol name="broken">
                 <interface name="ex_broken" version="1">
                   <event name="state"><arg name="value" type="uint" enum="ex_other.state"/></event>
                 </interface>
               </protocol>"#,
            "ex_other.state",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let error = scan(directory.path(), protocol).unwrap_err();
        assert!(error.to_string().contains(named), "{error}");
    }
}
