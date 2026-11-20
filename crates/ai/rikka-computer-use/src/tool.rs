use std::sync::Arc;
use std::time::Duration;

use rikka_computer::Computer;
use rikka_prompt::{Facts, Prompt};
use rikka_tool::{Function, Schema, Tool, Value};
use rikka_tool_docs::COMPUTER;
use rikka_tools::{Error, Result};

use crate::action::{self, ActionLimits};
use crate::batch::{self, Seen, Settling};

const POINTS: [&str; 2] = ["coordinate", "start_coordinate"];

const STRINGS: [&str; 2] = ["text", "scroll_direction"];

const NUMBERS: [&str; 2] = ["scroll_amount", "duration"];

pub(crate) fn declare(computer: &Computer, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let machine = prompt.stem() == COMPUTER;
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .param(
            "actions",
            Schema::array(action_schema(prompt, facts, machine)?)
                .describe(prompt.param("actions", facts)?),
        )
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let settling = Settling {
        idle: Duration::from_millis(prompt.limit("idle_timeout_ms")?),
        window: Duration::from_millis(prompt.limit("window_timeout_ms")?),
    };
    let limits = ActionLimits::read(prompt, machine)?;
    let seen = Arc::new(Seen::default());

    let computer = computer.clone();
    Ok(Function::new(tool, move |input: Value| {
        let computer = computer.clone();
        let seen = Arc::clone(&seen);
        async move {
            let steps = action::parse(input, limits)?;
            tokio::task::spawn_blocking(move || batch::run(&computer, &steps, settling, &seen))
                .await
                .map_err(|error| format!("computer action panicked: {error}"))
        }
    }))
}

fn action_schema(prompt: &Prompt, facts: &Facts, machine: bool) -> Result<Schema> {
    let described = |schema: Schema, name: &str| -> Result<Schema> {
        Ok(schema.describe(prompt.param(name, facts)?))
    };

    let mut schema = Schema::object().param("action", described(Schema::string(), "action")?);
    for name in POINTS {
        schema = schema.optional(name, described(Schema::array(Schema::integer()), name)?);
    }
    schema = schema
        .optional(
            "region",
            described(Schema::array(Schema::integer()), "region")?,
        )
        .optional("repeat", described(Schema::integer(), "repeat")?);
    for name in STRINGS {
        schema = schema.optional(name, described(Schema::string(), name)?);
    }
    for name in NUMBERS {
        schema = schema.optional(name, described(Schema::number(), name)?);
    }
    if machine {
        schema = schema
            .optional("width", described(Schema::integer(), "width")?)
            .optional("height", described(Schema::integer(), "height")?)
            .optional("desktop", described(Schema::boolean(), "desktop")?);
    }
    Ok(schema)
}
