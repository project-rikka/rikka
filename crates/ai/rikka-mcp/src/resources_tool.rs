use std::fmt::Write as _;

use rikka_agent::{Function, Reply};
use rikka_llm::{Tool, Value};
use serde::Deserialize;

use crate::contents::{Body, Contents};
use crate::docs::docs;
use crate::hub::Hub;
use crate::said::Said;
use crate::server_status::ServerStatus;

const ACTIONS: [&str; 2] = ["list", "read"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    List,
    Read,
}

#[derive(Debug, Deserialize)]
struct Args {
    action: Action,
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct Limits {
    listed: usize,
    read_chars: usize,
}

pub(crate) fn declare(hub: &Hub) -> Function {
    let docs = docs();
    let prompt = &docs.resources;
    let facts = &docs.facts;
    let description = |name: &str| {
        prompt
            .param(name, facts)
            .expect("MCP resources prompt should describe every parameter")
    };
    let limit = |name: &str| {
        usize::try_from(
            prompt
                .limit(name)
                .expect("MCP resources prompt should set its limits"),
        )
        .unwrap_or(usize::MAX)
    };

    let tool = Tool::new(prompt.name())
        .description(
            prompt
                .body(facts)
                .expect("MCP resources prompt should only reference supplied facts"),
        )
        .enumerated("action", ACTIONS, description("action"))
        .string("server", description("server"))
        .string("uri", description("uri"))
        .optional("server")
        .optional("uri")
        .build()
        .expect("MCP resources tool declaration should be valid");

    let limits = Limits {
        listed: limit("listed"),
        read_chars: limit("read_chars"),
    };
    let hub = hub.clone();
    Function::new(tool, move |input: Value| {
        let hub = hub.clone();
        async move { run(&hub, limits, input).await }
    })
}

async fn run(hub: &Hub, limits: Limits, input: Value) -> Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;
    let server = args
        .server
        .as_deref()
        .map(str::trim)
        .filter(|server| !server.is_empty());

    match args.action {
        Action::List => listing(hub, server, limits.listed).map(Reply::new),
        Action::Read => {
            let server = server.ok_or_else(|| "name the server".to_owned())?;
            let uri = args
                .uri
                .as_deref()
                .map(str::trim)
                .filter(|uri| !uri.is_empty())
                .ok_or_else(|| "name the resource's uri".to_owned())?;
            let contents = hub
                .read(server, uri)
                .await
                .map_err(|error| error.to_string())?;
            Ok(read(contents, limits.read_chars))
        }
    }
}

fn listing(hub: &Hub, server: Option<&str>, most: usize) -> Result<String, String> {
    let statuses: Vec<ServerStatus> = match server {
        Some(server) => {
            let standing = hub
                .status(server)
                .ok_or_else(|| format!("no MCP server named \"{server}\""))?;
            if !standing.is_connected() {
                return Err(format!("MCP server \"{server}\" is not connected"));
            }
            vec![standing]
        }
        None => hub
            .statuses()
            .into_iter()
            .filter(ServerStatus::is_connected)
            .collect(),
    };
    let mut listed = String::new();
    for standing in &statuses {
        served(&mut listed, standing, most).map_err(|error| error.to_string())?;
    }
    Ok(match listed.is_empty() {
        true => String::from("No connected MCP server is listed."),
        false => listed.trim_end().to_owned(),
    })
}

fn served(out: &mut String, standing: &ServerStatus, most: usize) -> std::fmt::Result {
    let resources = standing.resources();
    let templates = standing.resource_templates();
    writeln!(out, "## {}", standing.server())?;
    if resources.is_empty() && templates.is_empty() {
        return writeln!(out, "Offers no resources.\n");
    }
    if !resources.is_empty() {
        writeln!(out, "Resources:")?;
        for resource in resources.iter().take(most) {
            let named = resource.title().unwrap_or(resource.name());
            writeln!(
                out,
                "- {}{}",
                resource.uri(),
                about(named, resource.mime(), resource.description())
            )?;
        }
        if resources.len() > most {
            writeln!(out, "(and {} more)", resources.len() - most)?;
        }
    }
    if !templates.is_empty() {
        writeln!(out, "Resource templates:")?;
        for template in templates.iter().take(most) {
            let named = template.title().unwrap_or(template.name());
            writeln!(
                out,
                "- {}{}",
                template.uri_template(),
                about(named, template.mime(), template.description())
            )?;
        }
        if templates.len() > most {
            writeln!(out, "(and {} more)", templates.len() - most)?;
        }
    }
    writeln!(out)
}

fn about(name: &str, mime: Option<&str>, description: Option<&str>) -> String {
    let typed = mime.map(|mime| format!(" ({mime})")).unwrap_or_default();
    let described = description
        .and_then(|description| description.lines().next())
        .map(|line| format!(": {line}"))
        .unwrap_or_default();
    format!(" — {name}{typed}{described}")
}

fn read(contents: Vec<Contents>, most: usize) -> Reply {
    if contents.is_empty() {
        return Reply::new("the resource is empty");
    }
    let mut said = Said::default();
    let mut room = most;
    for contents in contents {
        let shortened = match contents.body() {
            Body::Text(text) => {
                let (kept, cut) = bounded(text, room);
                room = room.saturating_sub(kept.chars().count());
                (cut > 0).then(|| format!("{kept}\n[{cut} more characters were cut]"))
            }
            Body::Bytes(_) => None,
        };
        said.add_contents(match shortened {
            Some(text) => contents.with_text(text),
            None => contents,
        });
    }
    said.into_reply()
}

fn bounded(text: &str, room: usize) -> (&str, usize) {
    match text.char_indices().nth(room) {
        Some((at, _)) => (&text[..at], text[at..].chars().count()),
        None => (text, 0),
    }
}
