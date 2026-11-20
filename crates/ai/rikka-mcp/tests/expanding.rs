use std::collections::BTreeMap;

use rikka_mcp::{Error, Http, Oauth, Server, Stdio, Transport};

fn environment() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("TOKEN", "hunter2"),
        ("HOME", "/home/someone"),
        ("EMPTY", ""),
    ])
}

fn filled(server: &Server) -> Result<Server, Error> {
    let environment = environment();
    server.expanded(|name| environment.get(name).map(|value| (*value).to_owned()))
}

#[test]
fn references_expand_with_fallbacks() {
    let cases = [
        ("${TOKEN}", "hunter2"),
        ("Bearer ${TOKEN}", "Bearer hunter2"),
        ("${HOME}/src:${TOKEN}", "/home/someone/src:hunter2"),
        ("${MISSING:-plan b}", "plan b"),
        ("${EMPTY:-fallback}", "fallback"),
        ("${TOKEN:-unused}", "hunter2"),
        ("$TOKEN", "$TOKEN"),
        ("${not a name}", "${not a name}"),
        ("${UNCLOSED", "${UNCLOSED"),
        ("plain", "plain"),
    ];

    for (input, expected) in cases {
        let server = Server::new(
            "one",
            Http::new("https://example.invalid").with_header("X", input),
        );
        let expanded = filled(&server).expect("every reference should resolve");
        let Transport::Http(http) = expanded.transport() else {
            panic!("it stays remote");
        };
        assert_eq!(http.headers()["X"], expected, "{input}");
    }
}

#[test]
fn expands_all_fields_except_name() {
    let local = Server::new(
        "${TOKEN}",
        Stdio::new("${HOME}/bin/server")
            .arg("--key=${TOKEN}")
            .variable("KEY", "${TOKEN}")
            .cwd("${HOME}/work"),
    );
    let expanded = filled(&local).expect("references should expand");
    assert_eq!(expanded.name(), "${TOKEN}");
    let Transport::Stdio(stdio) = expanded.transport() else {
        panic!("it stays local");
    };
    assert_eq!(stdio.command(), "/home/someone/bin/server");
    assert_eq!(stdio.arguments(), ["--key=hunter2"]);
    assert_eq!(stdio.variables()["KEY"], "hunter2");
    assert_eq!(
        stdio.folder().map(|folder| folder.display().to_string()),
        Some("/home/someone/work".to_owned())
    );

    let remote = Server::new(
        "remote",
        Http::new("https://${TOKEN}.example.invalid/mcp")
            .with_oauth(Oauth::new().client("${TOKEN}").secret("${TOKEN}")),
    );
    let expanded = filled(&remote).expect("references should expand");
    let Transport::Http(http) = expanded.transport() else {
        panic!("it stays remote");
    };
    assert_eq!(http.url(), "https://hunter2.example.invalid/mcp");
    let oauth = http.oauth().expect("sign-in should be kept");
    assert_eq!(oauth.client_id(), Some("hunter2"));
    assert_eq!(oauth.client_secret(), Some("hunter2"));
}

#[test]
fn missing_references_are_all_reported() {
    let server = Server::new(
        "github",
        Stdio::new("npx")
            .variable("A", "${FIRST_MISSING}")
            .variable("B", "${SECOND_MISSING} and ${FIRST_MISSING}"),
    );

    match filled(&server) {
        Err(Error::Unset { names }) => {
            assert_eq!(names, ["FIRST_MISSING", "SECOND_MISSING"]);
        }
        other => panic!("{other:?}"),
    }
}
