use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt as _;
use rikka_agent::Function;
use rikka_llm::{Value, json};
use rikka_mcp::{Catalog, Hub, Place, Server, Stdio};
use tempfile::TempDir;

const MISSING: &str = "/nonexistent/rikka-mcp-test-server";

struct Ground {
    folder: TempDir,
    hub: Hub,
    tool: Function,
}

impl Ground {
    fn open() -> Ground {
        let folder = tempfile::tempdir().expect("temp dir should be created");
        let hub = Hub::builder()
            .place(
                Place::new("personal", folder.path().join("home/mcp.json"))
                    .with_description("every project of this person"),
            )
            .place(Place::new(
                "project",
                folder.path().join("project/.mcp.json"),
            ))
            .folder(folder.path())
            .build();
        let tool = hub.tool();

        Ground { folder, hub, tool }
    }

    fn path(&self, place: &str) -> std::path::PathBuf {
        self.hub
            .place(place)
            .expect("place should be declared")
            .path()
            .to_path_buf()
    }

    async fn call(&self, input: Value) -> Result<String, String> {
        self.tool
            .call(input)
            .await
            .map(|reply| reply.text().to_owned())
    }
}

fn adding(scope: Option<&str>) -> Value {
    let mut input = json!({
        "action": "add",
        "name": "tickets",
        "command": format!("{MISSING} --verbose"),
        "env": { "TICKETS_TOKEN": "${TICKETS_TOKEN}" },
    });
    if let Some(scope) = scope {
        input["scope"] = json!(scope);
    }
    input
}

#[tokio::test(flavor = "multi_thread")]
async fn add_with_multiple_scopes_requires_choice() {
    let ground = Ground::open();

    let refused = ground
        .call(adding(None))
        .await
        .expect_err("no scope is named");

    assert!(
        refused.contains("`personal`") && refused.contains("`project`"),
        "{refused}"
    );
    assert!(!ground.path("personal").exists());
    assert!(!ground.path("project").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn added_server_is_saved_and_reports_status() {
    let ground = Ground::open();

    let reply = ground
        .call(adding(Some("project")))
        .await
        .expect("server should be added");

    assert!(reply.contains("could not be connected"), "{reply}");
    let catalog = Catalog::read(&ground.path("project")).expect("catalog should be written");
    assert!(catalog.get("tickets").is_some());
    let standing = ground
        .hub
        .status("tickets")
        .expect("server status should exist");
    assert_eq!(standing.place(), Some("project"));
    assert!(standing.error().is_some());

    let removed = ground
        .call(json!({ "action": "remove", "name": "tickets" }))
        .await
        .expect("server should be removed");
    assert!(removed.contains("not declared"), "{removed}");
    assert!(ground.hub.status("tickets").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_hides_values() {
    let ground = Ground::open();
    let server = Server::new(
        "secrets",
        Stdio::new(MISSING)
            .variable("PLAIN", "hunter2")
            .variable("FROM_ENVIRONMENT", "${SOME_TOKEN}"),
    );
    ground
        .hub
        .save_catalog("personal", Catalog::from_servers([server]))
        .await
        .expect("file should be written");

    let listed = ground
        .call(json!({ "action": "list" }))
        .await
        .expect("listing should succeed");

    assert!(!listed.contains("hunter2"), "{listed}");
    assert!(listed.contains("PLAIN=(hidden)"), "{listed}");
    assert!(
        listed.contains("FROM_ENVIRONMENT=${SOME_TOKEN}"),
        "{listed}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn later_scope_overrides_name() {
    let ground = Ground::open();
    for place in ["personal", "project"] {
        ground
            .hub
            .save_catalog(
                place,
                Catalog::from_servers([Server::new(
                    "same",
                    Stdio::new(format!("{MISSING}-{place}")),
                )]),
            )
            .await
            .expect("file should be written");
    }

    let standing = ground
        .hub
        .status("same")
        .expect("server status should exist");
    assert_eq!(standing.place(), Some("project"));
    assert!(standing.error().unwrap_or_default().contains("project"));

    let listed = ground.call(json!({ "action": "list" })).await.unwrap();
    assert!(
        listed.contains("overridden by the one in `project`"),
        "{listed}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn edited_declaration_reloads_live() {
    let ground = Ground::open();
    ground.hub.start().await;
    let mut changes = std::pin::pin!(ground.hub.changes());

    write(
        &ground.path("personal"),
        &format!(r#"{{"mcpServers":{{"by-hand":{{"command":"{MISSING}"}}}}}}"#),
    );

    let noticed = tokio::time::timeout(Duration::from_secs(20), async {
        while ground.hub.status("by-hand").is_none() {
            changes.next().await;
        }
    })
    .await;
    assert!(noticed.is_ok(), "the change was never noticed");
    assert!(ground.folder.path().exists());
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("path should have a parent")).unwrap();
    std::fs::write(path, text).unwrap();
}
