use std::sync::atomic::{AtomicU64, Ordering};

use rikka_seam::{Request, SeamClient, SeamError};
use rikka_tool::{Function, Reply, Tool, ToolKit};
use serde_json::Value;

use crate::protocol::{
    CALL, CANCEL, CAPABILITY, Calling, Cancelling, Declared, Described, Outcome, VERSION, exposed,
};

static CALLS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn kit(described: &Described, client: &SeamClient) -> ToolKit {
    ToolKit::new(&described.kit)
        .with_description(&described.about)
        .with_instructions(&described.instructions)
        .with_functions(
            described.tools.iter().filter_map(|declared| {
                function(&described.kit, &described.title, declared, client)
            }),
        )
}

fn function(kit: &str, title: &str, declared: &Declared, client: &SeamClient) -> Option<Function> {
    let name = exposed(kit, &declared.name);
    let tool = Tool::from_schema(&name, declared.schema.clone())
        .description(&declared.description)
        .build()
        .inspect_err(|error| tracing::warn!("{title} offers invalid tool {name}: {error}"))
        .ok()?;

    let client = client.clone();
    let local = declared.name.clone();
    let title = title.to_owned();
    Some(Function::new(tool, move |input: Value| {
        called(client.clone(), local.clone(), title.clone(), input)
    }))
}

async fn called(
    client: SeamClient,
    tool: String,
    title: String,
    input: Value,
) -> Result<Reply, String> {
    let call = format!(
        "{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, Ordering::Relaxed)
    );
    let mut pending = Pending {
        client: client.clone(),
        call: call.clone(),
        completed: false,
    };

    let reply = client
        .call(
            Request::new(CAPABILITY, CALL)
                .require(VERSION)
                .encode(&Calling { call, tool, input }),
        )
        .await;
    pending.completed = true;

    match reply {
        Ok(payload) => payload
            .decode::<Outcome>()
            .map(Outcome::into_reply)
            .map_err(|error| format!("{title} answered with something unreadable: {error}")),
        Err(SeamError::Disconnected) => Err(format!("{title} closed before it answered")),
        Err(error) => Err(format!("{title} could not be reached: {error}")),
    }
}

struct Pending {
    client: SeamClient,
    call: String,
    completed: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.client.notify(
            Request::new(CAPABILITY, CANCEL)
                .require(VERSION)
                .encode(&Cancelling {
                    call: std::mem::take(&mut self.call),
                }),
        );
    }
}
