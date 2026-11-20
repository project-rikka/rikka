use rikka_mcp::{Catalog, Http, Server, Stdio, Transport};

const WRITTEN: &str = r#"{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "hunter2" }
    },
    "linear": {
      "type": "http",
      "url": "https://mcp.example.invalid/mcp",
      "headers": { "Authorization": "Bearer hunter2" },
      "oauth": { "clientId": "rikka-app", "clientSecret": "${LINEAR_SECRET}", "callbackPort": 8765 },
      "disabled": true
    }
  }
}"#;

#[test]
fn foreign_entries_round_trip() {
    let catalog = Catalog::from_json(WRITTEN).expect("catalog should parse");

    let github = catalog.get("github").expect("server should be declared");
    let Transport::Stdio(stdio) = github.transport() else {
        panic!("a command is a program to run");
    };
    assert_eq!(stdio.command(), "npx");
    assert_eq!(
        stdio.arguments(),
        ["-y", "@modelcontextprotocol/server-github"]
    );
    assert_eq!(stdio.variables()["GITHUB_TOKEN"], "hunter2");
    assert!(!github.is_disabled());

    let linear = catalog.get("linear").expect("server should be declared");
    let Transport::Http(http) = linear.transport() else {
        panic!("a URL is a server somewhere else");
    };
    assert_eq!(http.url(), "https://mcp.example.invalid/mcp");
    assert_eq!(http.headers()["Authorization"], "Bearer hunter2");
    let oauth = http.oauth().expect("sign-in should be declared");
    assert_eq!(oauth.client_id(), Some("rikka-app"));
    assert_eq!(oauth.client_secret(), Some("${LINEAR_SECRET}"));
    assert_eq!(oauth.callback_port(), Some(8765));
    assert!(linear.is_disabled());
}

#[test]
fn written_entries_round_trip() {
    let catalog = Catalog::from_json(WRITTEN).expect("catalog should parse");
    let again = Catalog::from_json(&catalog.to_json()).expect("written file should read back");
    assert_eq!(again, catalog);
}

#[test]
fn transport_is_inferred_and_sse_maps_to_http() {
    let catalog = Catalog::from_json(
        r#"{"mcpServers":{
            "old":{"url":"https://mcp.example.invalid/sse","type":"sse"},
            "plain":{"url":"https://mcp.example.invalid/mcp"},
            "run":{"command":"server"}
        }}"#,
    )
    .expect("catalog should parse");

    assert_eq!(catalog.get("old").unwrap().transport().kind(), "http");
    assert_eq!(catalog.get("plain").unwrap().transport().kind(), "http");
    assert_eq!(catalog.get("run").unwrap().transport().kind(), "stdio");
}

#[test]
fn project_overrides_settings_name() {
    let global = Catalog::from_servers([
        Server::new("github", Stdio::new("global-github")),
        Server::new("brave", Stdio::new("brave-search")),
    ]);
    let project = Catalog::from_servers([Server::new(
        "github",
        Http::new("https://mcp.example.invalid/mcp"),
    )]);

    let both = Catalog::merge([global, project]);

    assert_eq!(both.servers().len(), 2);
    assert_eq!(both.get("github").unwrap().transport().kind(), "http");
    assert_eq!(both.get("brave").unwrap().transport().kind(), "stdio");
}

#[test]
fn empty_catalog_removes_file() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let path = Catalog::path_in(folder.path());

    Catalog::from_servers([Server::new("github", Stdio::new("npx"))])
        .write(&path)
        .expect("file should be written");
    assert!(path.exists());
    assert_eq!(Catalog::read(&path).unwrap().servers().len(), 1);

    Catalog::new()
        .write(&path)
        .expect("empty catalog should be written");
    assert!(!path.exists());
    assert!(
        Catalog::read(&path)
            .expect("empty result should not be an error")
            .is_empty()
    );
}

#[test]
fn unknown_keys_survive_write() {
    let json = r#"{
      "$schema": "https://example.invalid/mcp.json",
      "inputs": [{ "id": "token", "type": "promptString" }],
      "mcpServers": {
        "github": {
          "command": "npx",
          "description": "the one at work",
          "alwaysAllow": ["search"]
        }
      }
    }"#;

    let again = Catalog::from_json(json)
        .expect("catalog should parse")
        .to_json();

    assert!(again.contains("$schema"), "{again}");
    assert!(again.contains("promptString"), "{again}");
    assert!(again.contains("the one at work"), "{again}");
    assert!(again.contains("alwaysAllow"), "{again}");
    assert_eq!(
        Catalog::from_json(&again).unwrap(),
        Catalog::from_json(json).unwrap()
    );
}

#[test]
fn file_with_other_keys_is_kept() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let path = Catalog::path_in(folder.path());

    let mut catalog = Catalog::from_json(
        r#"{"$schema":"https://example.invalid/x","mcpServers":{"one":{"command":"a"}}}"#,
    )
    .expect("catalog should parse");
    catalog.write(&path).expect("file should be written");

    assert!(catalog.remove("one"));
    catalog.write(&path).expect("file should be written");

    assert!(path.exists());
    let contents = std::fs::read_to_string(&path).expect("file should be readable");
    assert!(contents.contains("$schema"), "{contents}");
}

#[test]
fn malformed_file_is_error() {
    assert!(Catalog::from_json("not json at all").is_err());
    assert!(
        Catalog::from_json("   ")
            .expect("empty file should parse as empty catalog")
            .is_empty()
    );
}
