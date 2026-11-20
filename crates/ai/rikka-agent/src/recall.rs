use std::collections::{BTreeSet, HashMap, HashSet};

use rikka_llm::{Content, Message, ToolResult, Value};

use rikka_tool::Function;

const RECALLED: &str = "These tool results came before the summary and still apply:";

const UNLOADED: &str = "These deferred tools were loaded before the summary and are unloaded again; load them before calling them:";

const EACH: usize = 20_000;

const ALL: usize = 100_000;

const CUT: &str = "\n[cut short to fit after the summary; call the tool again for the rest]";

pub(crate) fn unloaded(folded: &[Message], retained: &[Message]) -> Option<String> {
    let still: BTreeSet<&str> = references(retained).collect();
    let gone: BTreeSet<&str> = references(folded)
        .filter(|name| !still.contains(name))
        .collect();
    (!gone.is_empty()).then(|| {
        let names: Vec<&str> = gone.into_iter().collect();
        format!("{UNLOADED} {}", names.join(", "))
    })
}

fn references(messages: &[Message]) -> impl Iterator<Item = &str> {
    messages
        .iter()
        .flat_map(Message::content)
        .filter_map(|content| match content {
            Content::ToolResult(result) => Some(result),
            _ => None,
        })
        .flat_map(ToolResult::references)
}

pub(crate) fn recalled(
    functions: &[Function],
    folded: &[Message],
    retained: &[Message],
) -> Option<String> {
    let lasting: HashSet<&str> = functions
        .iter()
        .filter(|function| function.is_lasting())
        .map(Function::name)
        .collect();
    if lasting.is_empty() {
        return None;
    }

    let still_seen: HashSet<(&str, String)> = lasting_results(retained, &lasting)
        .into_iter()
        .map(|(name, input, _)| (name, input))
        .collect();

    let mut seen = HashSet::new();
    let mut room = ALL;
    let mut entries = Vec::new();
    for (name, input, text) in lasting_results(folded, &lasting).into_iter().rev() {
        let key = (name, input);
        if still_seen.contains(&key) || !seen.insert(key) {
            continue;
        }
        let text = shortened(&text, EACH);
        if text.len() > room {
            continue;
        }
        room -= text.len();
        entries.push(format!("<recalled tool=\"{name}\">\n{text}\n</recalled>"));
    }

    (!entries.is_empty()).then(|| format!("{RECALLED}\n\n{}", entries.join("\n\n")))
}

fn lasting_results<'a>(
    messages: &'a [Message],
    lasting: &HashSet<&str>,
) -> Vec<(&'a str, String, String)> {
    let mut calls: HashMap<&str, (&str, &Value)> = HashMap::new();
    let mut results = Vec::new();
    for content in messages.iter().flat_map(Message::content) {
        match content {
            Content::ToolUse(call) if lasting.contains(call.name()) => {
                calls.insert(call.id(), (call.name(), call.input()));
            }
            Content::ToolResult(result) if !result.is_failed() => {
                if let Some(&(name, input)) = calls.get(result.id()) {
                    results.push((name, input.to_string(), result.text()));
                }
            }
            _ => {}
        }
    }
    results
}

fn shortened(text: &str, most: usize) -> String {
    if text.len() <= most {
        return text.to_owned();
    }
    let mut end = most.saturating_sub(CUT.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{CUT}", &text[..end])
}
