use std::collections::HashMap;
use std::fmt::{self, Write};

use rikka_llm::{Content, HostedPart, Message, Role, Value};

const DROPPED_SHARE: usize = 5;

pub(crate) fn render_transcript(messages: &[Message]) -> String {
    fmt::from_fn(|out| write_transcript(out, messages)).to_string()
}

fn write_transcript(summary: &mut impl Write, messages: &[Message]) -> fmt::Result {
    let mut called: HashMap<&str, &str> = HashMap::new();

    for message in messages {
        let who = speaker(message.role());
        for content in message.content() {
            match content {
                Content::Text(text) => {
                    writeln!(summary, "\n## {who}\n\n{}", text.as_str())?;
                }
                Content::ToolUse(call) => {
                    called.insert(call.id(), call.name());
                    ran(summary, who, call.name(), call.input())?;
                }
                Content::ToolResult(result) => {
                    let name = called.get(result.id()).copied().unwrap_or("a tool");
                    write_result(summary, name, result.is_failed(), &result.text())?;
                }
                Content::Image(_) => summary.write_str("\n## a picture\n")?,
                Content::Document(_) => summary.write_str("\n## a document\n")?,
                Content::Hosted(hosted) => match hosted.part() {
                    HostedPart::Call { tool, input, .. } => ran(summary, who, tool.name(), input)?,
                    HostedPart::Result { tool, outcome, .. } => write_result(
                        summary,
                        tool.name(),
                        outcome.is_failed(),
                        &outcome.to_string(),
                    )?,
                },
                _ => {}
            }
        }
    }
    Ok(())
}

fn ran(summary: &mut impl Write, who: &str, name: &str, input: &Value) -> fmt::Result {
    writeln!(summary, "\n## {who} runs {name}\n\n{input}")
}

fn write_result(summary: &mut impl Write, name: &str, failed: bool, output: &str) -> fmt::Result {
    let how = match failed {
        true => "failed",
        false => "answered",
    };
    writeln!(summary, "\n## {name} {how}\n\n{output}")
}

pub(crate) fn tag(summary: &str) -> String {
    let mut tag = String::from("conversation");
    while summary.contains(&format!("</{tag}>")) {
        tag.push_str("_x");
    }
    tag
}

pub(crate) fn formatted(text: &str) -> String {
    let drafted = without(text, "analysis");
    let summary = match tag_contents(&drafted, "summary") {
        Some(summary) => summary.to_owned(),
        None => drafted,
    };
    let summary = collapsed(summary.trim());
    match summary.is_empty() {
        true => text.trim().to_owned(),
        false => summary,
    }
}

pub(crate) fn shortened(messages: &[Message]) -> Option<&[Message]> {
    let rounds: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(at, message)| *at == 0 || message.role() == Role::Assistant)
        .map(|(at, _)| at)
        .collect();
    if rounds.len() < 2 {
        return None;
    }

    let dropped = (rounds.len() / DROPPED_SHARE).clamp(1, rounds.len() - 1);
    Some(&messages[rounds[dropped]..])
}

fn without(text: &str, tag: &str) -> String {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let Some(start) = text.find(&open) else {
        return text.to_owned();
    };
    match text[start..].find(&close) {
        Some(end) => format!("{}{}", &text[..start], &text[start + end + close.len()..]),
        None => text[..start].to_owned(),
    }
}

fn tag_contents<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let start = text.find(&open)? + open.len();
    let end = text[start..]
        .find(&close)
        .map_or(text.len(), |end| start + end);
    Some(&text[start..end])
}

fn collapsed(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut blank = 0;
    for line in text.lines() {
        match line.trim().is_empty() {
            true => blank += 1,
            false => {
                if !collapsed.is_empty() {
                    collapsed.push_str(if blank > 0 { "\n\n" } else { "\n" });
                }
                collapsed.push_str(line.trim_end());
                blank = 0;
            }
        }
    }
    collapsed
}

fn speaker(role: Role) -> &'static str {
    match role {
        Role::User => "The person",
        Role::Assistant => "You",
        Role::System => "An instruction",
    }
}
