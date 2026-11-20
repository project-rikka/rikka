use std::sync::{Arc, Mutex};

use animate_assistant::{ToolHost, ToolRequest, ToolResponse, functions};
use rikka_tool::{Function, Reply};
use serde_json::Value;

#[derive(Default)]
struct Recorder(Mutex<Option<ToolRequest>>);

impl ToolHost for Recorder {
    fn handle(&self, request: ToolRequest) -> ToolResponse {
        let reply = format!("{request:?}");
        *self.0.lock().expect("the recorder is not poisoned") = Some(request);
        Box::pin(async move { Ok(Reply::new(reply)) })
    }
}

pub(crate) fn declared(host: &Arc<dyn ToolHost>) -> Vec<Function> {
    functions(host).expect("every tool declares")
}

pub(crate) async fn ordered(tool: &str, input: Value) -> Result<ToolRequest, String> {
    let recorder = Arc::new(Recorder::default());
    let host: Arc<dyn ToolHost> = recorder.clone();
    let function = declared(&host)
        .into_iter()
        .find(|function| function.name() == tool)
        .expect("the tool is declared");
    function.call(input).await?;
    Ok(recorder
        .0
        .lock()
        .expect("the recorder is not poisoned")
        .take()
        .expect("the host was served"))
}
