use std::borrow::Cow;

use ade_prompt::{AGENT, SKILL};
use rikka_chat::{CallSummary, SummarizeCall};
use rikka_format::truncate;
use rikka_llm::Value;
use rikka_mcp::ToolCall;
use rikka_thread::Call;
use rikka_tools::ToolAction;
use rikka_ui::MaterialSymbol;

const GIST: usize = 96;

pub(crate) struct ToolCallLabels;

impl SummarizeCall for ToolCallLabels {
    fn summarize<'a>(&self, call: &'a Call) -> CallSummary<'a> {
        let deed = classify(call.tool(), call.input());
        if let Some(called) = ToolCall::parse(call.tool(), call.input()) {
            return served(called, call.tool(), deed);
        }
        CallSummary {
            name: deed.name(call.tool()).to_owned(),
            symbol: symbol(&deed),
            gist: gist(&deed).map(Cow::into_owned),
            pictures: matches!(deed, ToolAction::View { .. }),
            action: deed,
        }
    }
}

fn served<'a>(called: ToolCall<'_>, tool: &str, deed: ToolAction<'a>) -> CallSummary<'a> {
    let name = match called {
        ToolCall::Tool { server, tool } => format!("{server} · {tool}"),
        ToolCall::Managed { .. } | ToolCall::Resources { .. } => deed.name(tool).to_owned(),
    };
    CallSummary {
        name,
        symbol: MaterialSymbol::EXTENSION,
        gist: called.gist().map(|gist| shorten(&gist).into_owned()),
        pictures: false,
        action: deed,
    }
}

fn classify<'a>(tool: &str, input: &'a Value) -> ToolAction<'a> {
    ToolAction::classify(ade_prompt::naming(), tool, input)
}

fn symbol(deed: &ToolAction<'_>) -> MaterialSymbol {
    match deed {
        ToolAction::Tool { stem: AGENT, .. } => MaterialSymbol::HUB,
        ToolAction::Tool { stem: SKILL, .. } => MaterialSymbol::MENU_BOOK,
        other => rikka_chat::symbol(other),
    }
}

fn gist<'a>(deed: &'a ToolAction<'a>) -> Option<Cow<'a, str>> {
    match deed {
        ToolAction::Inspect { look, .. } => look.subject().map(shorten),
        ToolAction::View { path } => Some(shorten(path)),
        ToolAction::Search { query } => Some(shorten(query)),
        ToolAction::Tool { stem: AGENT, input } => labelled(input),
        ToolAction::Tool { stem: SKILL, input } => Some(shorten(string_field(input, "skill"))),
        ToolAction::Fetch { url } => Some(shorten(url)),
        ToolAction::Job { action, id } => Some(match id {
            Some(id) => Cow::Owned(format!("{action} {id}")),
            None => Cow::Borrowed(*action),
        }),
        ToolAction::Action { action, text } => Some(match text {
            Some(text) => Cow::Owned(shorten(&format!("{action} {text}")).into_owned()),
            None => shorten(action),
        }),
        ToolAction::Operate(operation) => {
            Some(Cow::Owned(shorten(&operation.to_string()).into_owned()))
        }
        ToolAction::Command { .. }
        | ToolAction::Write { .. }
        | ToolAction::Edit { .. }
        | ToolAction::Ask { .. }
        | ToolAction::Tool { .. }
        | ToolAction::Unknown => None,
    }
}

fn labelled(input: &Value) -> Option<Cow<'_, str>> {
    let labels: Vec<&str> = input
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|task| task.get("label").and_then(Value::as_str))
        .filter(|label| !label.trim().is_empty())
        .collect();

    match labels.is_empty() {
        true => None,
        false => Some(Cow::Owned(shorten(&labels.join(", ")).into_owned())),
    }
}

fn string_field<'a>(input: &'a Value, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn shorten(text: &str) -> Cow<'_, str> {
    truncate(text, GIST)
}
