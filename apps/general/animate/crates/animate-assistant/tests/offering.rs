use std::sync::Arc;

use animate_assistant::{ToolHost, ToolRequest, ToolResponse, offering};
use rikka_tool::Reply;
use serde_json::json;

use crate::support::{declared, ordered};

struct Idle;

impl ToolHost for Idle {
    fn handle(&self, _: ToolRequest) -> ToolResponse {
        Box::pin(async { Ok(Reply::new("done")) })
    }
}

#[tokio::test]
async fn offering_lists_instructions_and_tools() {
    assert!(offering("Animate").is_ok());
    let host: Arc<dyn ToolHost> = Arc::new(Idle);
    let functions = declared(&host);
    let names: Vec<&str> = functions.iter().map(|function| function.name()).collect();
    for name in [
        "Scene",
        "Pose",
        "Look",
        "EditGraph",
        "Materials",
        "Render",
        "SetCloth",
        "SetWind",
    ] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }
    let pose = functions
        .iter()
        .find(|function| function.name() == "Pose")
        .expect("declared");
    assert!(
        pose.tool().description().contains("Animate_"),
        "the model reads the names it calls: {}",
        pose.tool().description()
    );
    let schema = pose.tool().schema().to_string();
    assert!(schema.contains("additionalProperties"), "{schema}");

    assert!(ordered("Timeline", json!({ "frame": 30 })).await.is_ok());
    let error = ordered("Timeline", json!({}))
        .await
        .expect_err("nothing to do");
    assert!(error.contains("frame"), "{error}");
}
