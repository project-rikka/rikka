use std::os::unix::fs::PermissionsExt;

use rikka_remote_link::Pin;
use rikka_remote_wire::DEFAULT_PORT;
use viewer_hosts::{AddressError, Host, Hosts, Target};

#[test]
fn addresses_parse_with_and_without_ports() {
    let cases = [
        ("desk", Ok(("desk", DEFAULT_PORT))),
        (" 100.64.0.9:5000 ", Ok(("100.64.0.9", 5000))),
        (
            "nixos.tail1234.ts.net",
            Ok(("nixos.tail1234.ts.net", DEFAULT_PORT)),
        ),
        ("[fd7a:115c::1]:47901", Ok(("fd7a:115c::1", 47901))),
        ("fd7a:115c::1", Ok(("fd7a:115c::1", DEFAULT_PORT))),
        ("", Err(AddressError::Empty)),
        ("desk:0", Err(AddressError::Port("0".into()))),
        ("desk:http", Err(AddressError::Port("http".into()))),
        ("desk/../x", Err(AddressError::Name("desk/../x".into()))),
    ];
    for (text, expected) in cases {
        let parsed = Target::parse(text).map(|target| (target.host.clone(), target.port));
        assert_eq!(
            parsed,
            expected.map(|(host, port)| (host.to_owned(), port)),
            "{text:?}"
        );
    }
    assert_eq!(
        Target::parse("fd7a::1").unwrap().address(),
        format!("[fd7a::1]:{DEFAULT_PORT}")
    );
}

#[test]
fn hosts_persist_privately_and_list_by_last_use() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("viewer/hosts.json");
    let mut hosts = Hosts::default();
    for (address, used) in [("a:1", 5), ("b:1", 9), ("c:1", 1)] {
        hosts.remember(Host {
            address: address.into(),
            name: address.into(),
            pin: None,
            used,
        });
    }
    let pin = Pin::of_public_key(b"key");
    hosts.remember(Host {
        address: "c:1".into(),
        name: "renamed".into(),
        pin: Some(pin),
        used: 10,
    });
    hosts.save(&path).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut loaded = Hosts::load(&path).unwrap();
    let order: Vec<&str> = loaded
        .list()
        .iter()
        .map(|host| host.name.as_str())
        .collect();
    assert_eq!(order, ["renamed", "b:1", "a:1"]);
    assert_eq!(loaded.get("c:1").and_then(|host| host.pin), Some(pin));
    assert!(loaded.forget("b:1").is_some());
    assert!(loaded.get("b:1").is_none());
    assert_eq!(
        Hosts::load(&folder.path().join("none")).unwrap(),
        Hosts::default()
    );
}
