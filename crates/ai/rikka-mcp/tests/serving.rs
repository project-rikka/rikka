use rikka_llm::json;
use rikka_mcp::{Body, Catalog, Error, Hub, Part, Place, Server, Stdio, ToolCall};
use rikka_test_mcp::echo_server;
use tempfile::TempDir;

async fn connected() -> (TempDir, Hub) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let hub = Hub::builder()
        .place(Place::new("assistant", folder.path().join("mcp.json")))
        .folder(folder.path())
        .build();
    hub.save_catalog(
        "assistant",
        Catalog::from_servers([Server::new("echo", Stdio::new(echo_server(folder.path())))]),
    )
    .await
    .expect("file should be written");
    let standing = hub.status("echo").expect("server status should exist");
    assert!(standing.is_connected(), "{:?}", standing.error());
    (folder, hub)
}

#[tokio::test(flavor = "multi_thread")]
async fn connected_server_exposes_kit() {
    let (_folder, hub) = connected().await;

    let standing = hub.status("echo").expect("server status should exist");
    assert_eq!(standing.title(), Some("Echo"));
    assert_eq!(standing.instructions(), Some("Say things back."));
    assert_eq!(standing.tool_count(), 1);

    let kits = hub.toolkit_set().kits();
    let kit = kits
        .iter()
        .find(|kit| kit.name() == "echo")
        .expect("echo kit should be listed");
    assert_eq!(kit.instructions(), "Say things back.");
    let function = kit.functions().first().expect("kit should have a tool");
    let input = json!({ "text": "hello" });
    assert_eq!(
        ToolCall::parse(function.name(), &input),
        Some(ToolCall::Tool {
            server: "echo",
            tool: "say"
        })
    );

    let reply = function.call(input).await.expect("server should reply");
    assert_eq!(reply.text(), "echoed");

    let instruction = hub.instruction();
    assert!(
        instruction.contains("- echo: Echo *resources*"),
        "{instruction}"
    );

    hub.save_catalog("assistant", Catalog::new())
        .await
        .expect("file should be written");
    assert!(hub.toolkit_set().kits().is_empty());
    assert!(hub.statuses().is_empty());
    assert!(hub.prompt("echo:greet").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn prompt_fills_arguments_in_order() {
    let (_folder, hub) = connected().await;

    let prompt = hub.prompt("echo:greet").expect("prompt should be listed");
    assert_eq!(prompt.server(), "echo");
    assert_eq!(prompt.name(), "greet");
    let arguments: Vec<(&str, bool)> = prompt
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.is_required()))
        .collect();
    assert_eq!(arguments, [("who", true), ("mood", false)]);

    let prompted = hub
        .get_prompt(
            "echo:greet",
            &["Ada".to_owned(), "very".to_owned(), "warmly".to_owned()],
        )
        .await
        .expect("prompt should be filled");
    let [Part::Text(text), Part::Contents(contents)] = prompted.parts() else {
        panic!("unexpected parts: {:?}", prompted.parts());
    };
    assert_eq!(text, "Greet Ada, very warmly");
    assert_eq!(contents.uri(), "note://hello");
    assert_eq!(contents.body(), &Body::Text("hello there".to_owned()));

    let unfilled = hub.get_prompt("echo:greet", &[]).await;
    assert!(
        matches!(&unfilled, Err(Error::Unfilled { argument, .. }) if argument == "who"),
        "{unfilled:?}"
    );

    let offered = hub
        .complete("echo:greet", &[], "A")
        .await
        .expect("server should complete");
    assert_eq!(offered, ["Ada", "Alan"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn resource_mention_reads_contents() {
    let (_folder, hub) = connected().await;

    let standing = hub.status("echo").expect("server status should exist");
    let resources = standing.resources();
    let [resource] = &resources[..] else {
        panic!("unexpected resources: {resources:?}");
    };
    assert_eq!(resource.mention(), "echo:note://hello");
    let templates = standing.resource_templates();
    assert_eq!(
        templates
            .iter()
            .map(|template| (template.mention_prefix(), template.mention_template()))
            .collect::<Vec<(&str, &str)>>(),
        [("echo:note://", "echo:note://{name}")]
    );

    assert!(hub.is_resource_mention("echo:note://anything"));
    assert!(!hub.is_resource_mention("other:note://hello"));
    let contents = hub
        .read_mention(resource.mention())
        .await
        .expect("resource should be read");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0].mime(), Some("text/plain"));

    let tool = hub.resources_tool();
    let listed = tool
        .call(json!({ "action": "list" }))
        .await
        .expect("resources should be listed");
    assert!(listed.text().contains("note://hello"), "{}", listed.text());
    assert!(listed.text().contains("note://{name}"), "{}", listed.text());
    let read = tool
        .call(json!({ "action": "read", "server": "echo", "uri": "note://hello" }))
        .await
        .expect("resource should be read");
    assert_eq!(read.text(), "note://hello:\nhello there");
}
