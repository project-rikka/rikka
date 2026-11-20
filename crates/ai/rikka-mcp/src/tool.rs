use std::collections::BTreeMap;
use std::fmt;

use rikka_agent::{Function, Reply};
use rikka_llm::{Schema, Tool, Value, json};
use serde::Deserialize;

use crate::catalog::Catalog;
use crate::docs::docs;
use crate::http::Http;
use crate::hub::Hub;
use crate::oauth::Oauth;
use crate::place::Place;
use crate::server::Server;
use crate::server_status::ServerStatus;
use crate::stdio::Stdio;
use crate::transport::Transport;

const ACTIONS: [&str; 8] = [
    "list",
    "add",
    "remove",
    "enable",
    "disable",
    "sign_in",
    "sign_out",
    "reconnect",
];

const HIDDEN: &str = "(hidden)";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    List,
    Add,
    Remove,
    Enable,
    Disable,
    SignIn,
    SignOut,
    Reconnect,
}

#[derive(Debug, Deserialize)]
struct Args {
    action: Action,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    oauth: Option<Signing>,
}

#[derive(Debug, Deserialize)]
struct Signing {
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    callback_port: Option<u16>,
}

pub(crate) fn declare(hub: &Hub) -> Function {
    let docs = docs();
    let prompt = &docs.tool;
    let mut facts = docs.facts.clone();
    facts.set("scopes", scopes(hub.places()));
    facts.set(
        "scope_names",
        hub.places()
            .iter()
            .map(|place| format!("`{}`", place.name()))
            .collect::<Vec<String>>()
            .join(", "),
    );
    let description = |name: &str| {
        prompt
            .param(name, &facts)
            .expect("MCP tool prompt should describe every parameter")
    };
    let names: Vec<&str> = hub.places().iter().map(Place::name).collect();
    let pairs = |name: &str| {
        Schema::raw(json!({
            "type": "object",
            "additionalProperties": { "type": "string" },
        }))
        .describe(description(name))
    };

    let tool = Tool::new(prompt.name())
        .description(
            prompt
                .body(&facts)
                .expect("MCP tool prompt should only reference supplied facts"),
        )
        .enumerated("action", ACTIONS, description("action"))
        .string("name", description("name"))
        .param(
            "scope",
            Schema::string()
                .enumerate(names)
                .describe(description("scope")),
        )
        .string("command", description("command"))
        .string("cwd", description("cwd"))
        .param("env", pairs("env"))
        .string("url", description("url"))
        .param("headers", pairs("headers"))
        .param(
            "oauth",
            Schema::object()
                .optional("client_id", Schema::string())
                .optional("client_secret", Schema::string())
                .optional("callback_port", Schema::integer())
                .describe(description("oauth")),
        )
        .optional("name")
        .optional("scope")
        .optional("command")
        .optional("cwd")
        .optional("env")
        .optional("url")
        .optional("headers")
        .optional("oauth")
        .build()
        .expect("MCP tool declaration should be valid");

    let most = usize::try_from(
        prompt
            .limit("listed_tools")
            .expect("MCP tool prompt should set a listing limit"),
    )
    .unwrap_or(usize::MAX);
    let hub = hub.clone();
    Function::new(tool, move |input: Value| {
        let hub = hub.clone();
        async move { run(&hub, most, input).await }
    })
}

async fn run(hub: &Hub, most: usize, input: Value) -> Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;
    let scope = args.scope.as_deref();

    match args.action {
        Action::List => return Ok(Reply::new(listing(hub))),
        Action::Add => add(hub, named(&args)?, &args).await?,
        Action::Remove => remove(hub, named(&args)?, scope).await?,
        Action::Enable => switch(hub, named(&args)?, scope, true).await?,
        Action::Disable => switch(hub, named(&args)?, scope, false).await?,
        Action::SignIn => hub
            .sign_in(named(&args)?)
            .await
            .map_err(|error| error.to_string())?,
        Action::SignOut => hub
            .sign_out(named(&args)?)
            .await
            .map_err(|error| error.to_string())?,
        Action::Reconnect => hub
            .reconnect(named(&args)?)
            .await
            .map_err(|error| error.to_string())?,
    }
    Ok(Reply::new(stands(hub, named(&args)?, most)))
}

fn named(args: &Args) -> Result<&str, String> {
    args.name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "name the server".to_owned())
}

async fn add(hub: &Hub, name: &str, args: &Args) -> Result<(), String> {
    let place = resolve_place(hub, args.scope.as_deref())?;
    let transport = transport(args)?;
    let mut catalog = hub
        .catalog(place.name())
        .map_err(|error| error.to_string())?;
    let previous = catalog.get(name).cloned();

    let server = Server::new(name, transport).with_extra(
        previous
            .as_ref()
            .map(|previous| previous.extra().clone())
            .unwrap_or_default(),
    );
    if let Some(nonsense) = server.config_error() {
        return Err(format!("the server cannot be kept: {nonsense}"));
    }
    if previous.as_ref() == Some(&server) {
        return Ok(());
    }

    catalog.insert(server);
    keep(hub, place, catalog).await
}

async fn remove(hub: &Hub, name: &str, scope: Option<&str>) -> Result<(), String> {
    let place = holding(hub, name, scope)?;
    let mut catalog = hub
        .catalog(place.name())
        .map_err(|error| error.to_string())?;
    if catalog.get(name).is_none() {
        return Err(format!("\"{name}\" is not kept in `{}`", place.name()));
    }
    catalog.remove(name);
    keep(hub, place, catalog).await
}

async fn switch(hub: &Hub, name: &str, scope: Option<&str>, on: bool) -> Result<(), String> {
    let place = holding(hub, name, scope)?;
    let mut catalog = hub
        .catalog(place.name())
        .map_err(|error| error.to_string())?;
    let server = catalog
        .get(name)
        .cloned()
        .ok_or_else(|| format!("\"{name}\" is not kept in `{}`", place.name()))?;
    if server.is_disabled() != on {
        return Ok(());
    }

    catalog.insert(server.with_disabled(!on));
    keep(hub, place, catalog).await
}

async fn keep(hub: &Hub, place: &Place, catalog: Catalog) -> Result<(), String> {
    hub.save_catalog(place.name(), catalog)
        .await
        .map_err(|error| error.to_string())
}

fn resolve_place<'a>(hub: &'a Hub, scope: Option<&str>) -> Result<&'a Place, String> {
    match (scope, hub.places()) {
        (Some(scope), _) => hub.place(scope).ok_or_else(|| unplaced(hub, scope)),
        (None, [only]) => Ok(only),
        (None, _) => Err(format!(
            "say which scope keeps it: {}",
            named_places(hub.places())
        )),
    }
}

fn holding<'a>(hub: &'a Hub, name: &str, scope: Option<&str>) -> Result<&'a Place, String> {
    if let Some(scope) = scope {
        return hub.place(scope).ok_or_else(|| unplaced(hub, scope));
    }
    match hub.declared_in(name).as_slice() {
        [] => Err(format!("no MCP server named \"{name}\"")),
        [only] => Ok(only),
        many => Err(format!(
            "\"{name}\" is kept in {}; say which scope",
            named_places(many.iter().copied())
        )),
    }
}

fn unplaced(hub: &Hub, scope: &str) -> String {
    format!(
        "there is no scope called \"{scope}\"; the scopes are {}",
        named_places(hub.places())
    )
}

fn named_places<'a>(places: impl IntoIterator<Item = &'a Place>) -> String {
    places
        .into_iter()
        .map(|place| format!("`{}`", place.name()))
        .collect::<Vec<String>>()
        .join(", ")
}

fn transport(args: &Args) -> Result<Transport, String> {
    let command = args
        .command
        .as_deref()
        .map(str::trim)
        .filter(|it| !it.is_empty());
    let url = args
        .url
        .as_deref()
        .map(str::trim)
        .filter(|it| !it.is_empty());

    match (command, url) {
        (Some(_), Some(_)) => Err("give either a command or a URL, not both".to_owned()),
        (None, None) => Err("give the command that starts the server, or its URL".to_owned()),
        (Some(command), None) => {
            if !args.headers.is_empty() || args.oauth.is_some() {
                return Err("headers and oauth belong to remote servers".to_owned());
            }
            let mut stdio = Stdio::parse(command)
                .map_err(|error| error.to_string())?
                .env(args.env.clone());
            if let Some(cwd) = args
                .cwd
                .as_deref()
                .map(str::trim)
                .filter(|it| !it.is_empty())
            {
                stdio = stdio.cwd(cwd);
            }
            Ok(stdio.into())
        }
        (None, Some(url)) => {
            if !args.env.is_empty() || args.cwd.is_some() {
                return Err("env and cwd belong to local servers".to_owned());
            }
            let mut http = Http::new(url).with_headers(args.headers.clone());
            if let Some(signing) = &args.oauth {
                let mut oauth = Oauth::new();
                if let Some(client) = &signing.client_id {
                    oauth = oauth.client(client.as_str());
                }
                if let Some(secret) = &signing.client_secret {
                    oauth = oauth.secret(secret.as_str());
                }
                if let Some(port) = signing.callback_port {
                    oauth = oauth.port(port);
                }
                http = http.with_oauth(oauth);
            }
            Ok(http.into())
        }
    }
}

fn stands(hub: &Hub, name: &str, most: usize) -> String {
    let Some(standing) = hub.status(name) else {
        return format!("\"{name}\" is not declared anywhere now.");
    };
    let location = standing
        .place()
        .map(|place| format!(" (kept in `{place}`)"))
        .unwrap_or_default();

    if !standing.is_started() {
        return format!("\"{name}\"{location} is switched off.");
    }
    if standing.needs_sign_in() {
        return format!(
            "\"{name}\"{location} needs the person to sign in. Tell them, then call `sign_in`."
        );
    }
    if let Some(trouble) = standing.error() {
        return format!("\"{name}\"{location} could not be connected: {trouble}");
    }

    let offers = standing.tools();
    fmt::from_fn(|out| {
        write!(
            out,
            "\"{name}\"{location} is connected and offers {} tool(s)",
            offers.len()
        )?;
        if offers.is_empty() {
            return Ok(());
        }
        let listed: Vec<&str> = offers.iter().take(most).map(|offer| offer.name()).collect();
        write!(out, ": {}", listed.join(", "))?;
        if offers.len() > listed.len() {
            write!(out, " and {} more", offers.len() - listed.len())?;
        }
        write!(
            out,
            ". Load them with `{}` `select:{name}`.",
            docs().facts.get("tool_search_tool").unwrap_or("ToolSearch")
        )
    })
    .to_string()
}

fn listing(hub: &Hub) -> String {
    let standings = hub.statuses();
    let listing = fmt::from_fn(|out| {
        for place in hub.places() {
            write_place(out, hub, &standings, place)?;
        }
        Ok(())
    })
    .to_string();
    listing.trim_end().to_owned()
}

fn write_place(
    out: &mut fmt::Formatter<'_>,
    hub: &Hub,
    standings: &[ServerStatus],
    place: &Place,
) -> fmt::Result {
    writeln!(out, "`{}` ({}):", place.name(), place.path().display())?;
    let catalog = match hub.catalog(place.name()) {
        Ok(catalog) => catalog,
        Err(error) => return writeln!(out, "  {error}"),
    };
    if catalog.is_empty() {
        writeln!(out, "  no servers")?;
    }
    for server in catalog.servers() {
        let standing = standings
            .iter()
            .find(|standing| standing.server() == server.name());
        let state = match standing {
            _ if server.is_disabled() => "switched off".to_owned(),
            Some(standing) if standing.place() != Some(place.name()) => format!(
                "overridden by the one in `{}`",
                standing.place().unwrap_or_default()
            ),
            Some(standing) if standing.needs_sign_in() => "needs sign-in".to_owned(),
            Some(standing) => match standing.error() {
                Some(trouble) => format!("not connected: {trouble}"),
                None => format!("connected, {} tool(s)", standing.tool_count()),
            },
            None => "not connected yet".to_owned(),
        };
        writeln!(
            out,
            "- {} [{}] {}{} — {state}",
            server.name(),
            server.transport().kind(),
            server.transport(),
            carried(server.transport()),
        )?;
    }
    Ok(())
}

fn carried(transport: &Transport) -> String {
    let (label, values) = match transport {
        Transport::Stdio(stdio) => ("env", stdio.variables()),
        Transport::Http(http) => ("headers", http.headers()),
    };
    fmt::from_fn(|out| {
        if !values.is_empty() {
            let masked_values: Vec<String> = values
                .iter()
                .map(|(name, value)| format!("{name}={}", masked(value)))
                .collect();
            write!(out, "; {label}: {}", masked_values.join(", "))?;
        }
        if let Transport::Http(http) = transport
            && let Some(oauth) = http.oauth()
        {
            if let Some(client) = oauth.client_id() {
                write!(out, "; oauth client {client}")?;
            }
            if let Some(port) = oauth.callback_port() {
                write!(out, "; oauth callback port {port}")?;
            }
        }
        Ok(())
    })
    .to_string()
}

fn masked(value: &str) -> &str {
    let mut rest = value.trim();
    if rest.is_empty() {
        return HIDDEN;
    }
    while let Some(remainder) = rest.strip_prefix("${") {
        let Some(end) = remainder.find('}') else {
            return HIDDEN;
        };
        rest = remainder[end + 1..].trim_start();
    }
    match rest.is_empty() {
        true => value,
        false => HIDDEN,
    }
}

fn scopes(places: &[Place]) -> String {
    places
        .iter()
        .map(|place| {
            let about = place.description();
            match about.is_empty() {
                true => format!("- `{}`: {}", place.name(), place.path().display()),
                false => format!("- `{}`: {about} ({})", place.name(), place.path().display()),
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}
