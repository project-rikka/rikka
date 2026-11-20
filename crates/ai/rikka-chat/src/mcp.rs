use futures_util::future::join_all;
use rikka_llm::Media;
use rikka_mcp::{Body, Contents, Error, Hub, Part};
use rikka_thread::{Ask, Attachment};

use crate::argument::Argument;
use crate::attaching::{Attached, read};
use crate::chat::Labels;
use crate::command::invoked;
use crate::mention::mentioned;
use crate::offer::Offer;
use crate::prelude::*;
use crate::words::words;

const OFFERED: usize = 50;

const NTH: char = '#';

#[derive(Clone, Debug)]
pub struct Gathered {
    pub ask: Ask,
    pub attached: Attached,
    pub mentions: Vec<String>,
}

#[must_use]
pub fn prompt_offers(hub: &Hub, typed: &str) -> Vec<Offer> {
    let typed = typed.to_lowercase();
    hub.statuses()
        .iter()
        .flat_map(|standing| {
            let prompts = standing.prompts();
            let found: Vec<Offer> = prompts
                .iter()
                .filter(|prompt| {
                    prompt.command().to_lowercase().starts_with(&typed)
                        || prompt.name().to_lowercase().starts_with(&typed)
                })
                .take(OFFERED)
                .map(Offer::prompt)
                .collect();
            found
        })
        .take(OFFERED)
        .collect()
}

#[must_use]
pub fn resource_offers(hub: &Hub, typed: &str) -> Vec<Offer> {
    let typed = typed.to_lowercase();
    let statuses = hub.statuses();
    let resources = statuses.iter().flat_map(|standing| {
        let resources = standing.resources();
        let found: Vec<Offer> = resources
            .iter()
            .filter(|resource| {
                matches(
                    resource.mention(),
                    resource.name(),
                    resource.title(),
                    &typed,
                )
            })
            .take(OFFERED)
            .map(Offer::resource)
            .collect();
        found
    });
    let templates = statuses.iter().flat_map(|standing| {
        let templates = standing.resource_templates();
        let found: Vec<Offer> = templates
            .iter()
            .filter(|template| {
                matches(
                    template.mention_prefix(),
                    template.name(),
                    template.title(),
                    &typed,
                )
            })
            .take(OFFERED)
            .map(Offer::template)
            .collect();
        found
    });
    resources.chain(templates).take(OFFERED).collect()
}

pub async fn argument_offers(hub: Hub, argument: Argument) -> Vec<Offer> {
    let Some(prompt) = hub.prompt(argument.command()) else {
        return Vec::new();
    };
    let Some(asked) = prompt.arguments().get(argument.given().len()) else {
        return Vec::new();
    };
    match hub
        .complete(argument.command(), argument.given(), argument.typed())
        .await
    {
        Ok(values) => values
            .iter()
            .take(OFFERED)
            .map(|value| Offer::value(value, asked))
            .collect(),
        Err(error) => {
            tracing::warn!("failed to complete /{}: {error}", argument.command());
            Vec::new()
        }
    }
}

#[must_use]
pub fn needs_mcp(hub: &Hub, ask: &Ask) -> bool {
    prompted(hub, ask).is_some()
        || mentioned(ask.sent_text())
            .iter()
            .any(|mention| hub.is_resource_mention(mention))
}

pub async fn gather<C: Labels>(hub: &Hub, ask: Ask) -> Result<Gathered, String> {
    let (ask, attached, mentions) = match prompted(hub, &ask) {
        Some(command) => {
            let mentions = mentioned(ask.text());
            let (ask, attached) = expand::<C>(hub, ask, &command).await?;
            (ask, attached, mentions)
        }
        None => {
            let mentions = mentioned(ask.sent_text());
            (ask, Attached::default(), mentions)
        }
    };
    let (resources, mentions): (Vec<String>, Vec<String>) = mentions
        .into_iter()
        .partition(|mention| hub.is_resource_mention(mention));
    let read = read_resources::<C>(hub, &resources).await;
    Ok(Gathered {
        ask,
        attached: attached.join(read),
        mentions,
    })
}

fn prompted(hub: &Hub, ask: &Ask) -> Option<String> {
    let invoked = invoked(ask.text()).filter(|_| ask.text() == ask.sent_text())?;
    hub.prompt(invoked.name())
        .map(|prompt| prompt.command().to_owned())
}

async fn expand<C: Labels>(hub: &Hub, ask: Ask, command: &str) -> Result<(Ask, Attached), String> {
    let arguments = invoked(ask.text()).map_or_else(Vec::new, |invoked| words(invoked.arguments()));
    let prompted = hub
        .get_prompt(command, &arguments)
        .await
        .map_err(|error| unprompted(command, &error))?;

    let mut texts = Vec::new();
    let mut attachments = Vec::new();
    for (nth, part) in prompted.into_parts().into_iter().enumerate() {
        match part {
            Part::Text(text) => texts.push(text),
            Part::Picture { media, data } => attachments.push(
                Attachment::new(
                    format!("/{command}/{}.{}", nth + 1, media.extension()),
                    media,
                    data,
                )
                .map_err(|error| error.to_string()),
            ),
            Part::Contents(contents) => {
                let name = format!("/{command}/{}", contents.uri());
                attachments.push(attached(name, contents));
            }
        }
    }
    if texts.iter().all(|text| text.trim().is_empty()) && attachments.is_empty() {
        return Err(tr!("chat.mcp.unsaid", command = command).to_string());
    }
    Ok((
        ask.with_expansion(texts.join("\n\n")),
        read::<C>(attachments),
    ))
}

async fn read_resources<C: Labels>(hub: &Hub, mentions: &[String]) -> Attached {
    let fetched = join_all(mentions.iter().map(|mention| hub.read_mention(mention))).await;
    let mut attachments = Vec::new();
    for (mention, contents) in mentions.iter().zip(fetched) {
        match contents {
            Ok(contents) if contents.is_empty() => {
                attachments.push(Err(
                    tr!("chat.mcp.empty", mention = mention.as_str()).to_string()
                ));
            }
            Ok(contents) => {
                let single = contents.len() == 1;
                for (nth, contents) in contents.into_iter().enumerate() {
                    let name = match single {
                        true => mention.clone(),
                        false => format!("{mention}{NTH}{}", nth + 1),
                    };
                    attachments.push(attached(name, contents));
                }
            }
            Err(error) => attachments.push(Err(error.to_string())),
        }
    }
    read::<C>(attachments)
}

#[must_use]
pub fn kept_attachments(said: &str, sent: &str, attachments: &[Attachment]) -> Vec<Attachment> {
    let mentions: Vec<String> = mentioned(sent).into_iter().chain(mentioned(said)).collect();
    let prompted = invoked(said).map(|invoked| format!("/{}/", invoked.name()));
    attachments
        .iter()
        .filter(|attachment| {
            let name = attachment.name();
            let from_mention = mentions.iter().any(|mention| {
                name == mention
                    || name
                        .strip_prefix(mention.as_str())
                        .is_some_and(|rest| rest.starts_with(NTH))
            });
            let from_prompt = prompted
                .as_deref()
                .is_some_and(|prefix| name.starts_with(prefix));
            !from_mention && !from_prompt
        })
        .cloned()
        .collect()
}

fn attached(name: String, contents: Contents) -> Result<Attachment, String> {
    let mime = contents.mime().map(str::to_owned);
    match contents.into_body() {
        Body::Text(text) => Attachment::new(name, Media::PlainText, text.into_bytes())
            .map_err(|error| error.to_string()),
        Body::Bytes(bytes) => {
            let media = mime.as_deref().and_then(Media::from_mime).or_else(|| {
                std::str::from_utf8(&bytes)
                    .is_ok()
                    .then_some(Media::PlainText)
            });
            match media {
                Some(media) => {
                    Attachment::new(name, media, bytes).map_err(|error| error.to_string())
                }
                None => Err(tr!(
                    "chat.mcp.unattachable",
                    name = name.as_str(),
                    kind = mime.as_deref().unwrap_or("data")
                )
                .to_string()),
            }
        }
    }
}

fn unprompted(command: &str, error: &Error) -> String {
    match error {
        Error::Unfilled { argument, .. } => tr!(
            "chat.mcp.unfilled",
            command = command,
            argument = argument.as_str()
        ),
        Error::Unasked(_) => tr!("chat.mcp.unasked", command = command),
        other => tr!(
            "chat.mcp.failed",
            command = command,
            reason = &other.to_string()
        ),
    }
    .to_string()
}

fn matches(mention: &str, name: &str, title: Option<&str>, typed: &str) -> bool {
    mention.to_lowercase().starts_with(typed)
        || (!typed.is_empty()
            && [Some(name), title]
                .into_iter()
                .flatten()
                .any(|named| named.to_lowercase().contains(typed)))
}
