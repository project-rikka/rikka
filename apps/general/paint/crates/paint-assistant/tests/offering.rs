use std::sync::Arc;

use paint_assistant::{Order, ToolHost, ToolResponse, functions, offering};
use rikka_tool::Reply;
use serde_json::json;

struct Echo;

impl ToolHost for Echo {
    fn handle(&self, order: Order) -> ToolResponse {
        Box::pin(async move { Ok(Reply::new(format!("{order:?}"))) })
    }
}

#[tokio::test]
async fn every_tool_parses_calls_into_orders() {
    assert!(offering("Paint").is_ok());
    let host: Arc<dyn ToolHost> = Arc::new(Echo);
    let functions = functions(&host).expect("every tool declares");
    let names: Vec<&str> = functions.iter().map(|function| function.name()).collect();
    for name in ["Document", "View", "Measure", "Adjust", "Draw", "Save"] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }
    let adjust = functions
        .iter()
        .find(|function| function.name() == "Adjust")
        .expect("declared");
    assert!(!adjust.tool().description().is_empty());
    let schema = adjust.tool().schema().to_string();
    assert!(schema.contains("gaussian_blur"), "{schema}");

    let served = adjust
        .call(
            json!({ "name": "Warmer", "steps": [{ "op": "white_balance", "temperature": 5600 }] }),
        )
        .await
        .expect("an order");
    assert!(served.text().contains("WhiteBalance"), "{}", served.text());
    let refused = adjust
        .call(json!({ "name": "Warmer", "steps": [] }))
        .await
        .expect_err("nothing to do");
    assert!(refused.contains("no steps"), "{refused}");
}
