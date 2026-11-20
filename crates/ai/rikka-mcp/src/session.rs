use rikka_sync::Mutex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_agent::{Function, Reply};
use rikka_llm::Value;
use rikka_net::{HeaderMap, HeaderName, HeaderValue, header};
use serde_json::Map;
use tokio::io::AsyncBufReadExt;
use tokio::process::{ChildStderr, Command};

use crate::client::{Connection, Failure, Link, LinkError};
use crate::contents::Contents;
use crate::credentials::{Signing, SigningError};
use crate::declaration::Declared;
use crate::http::Http;
use crate::listing::{Listed, Lister, Listing};
use crate::outcome;
use crate::prompted::Prompted;
use crate::protocol::{
    CallToolParams, CallToolResult, CompleteParams, CompleteResult, GetPromptParams,
    GetPromptResult, InitializeResult, ReadResourceParams, ReadResourceResult, method,
};
use crate::server::Server;
use crate::server_status::{Condition, Peer};
use crate::stdio::Stdio;
use crate::transport::Transport;
use crate::vault::Vault;
use crate::watch::Watch;
use crate::watcher::Watcher;

const OPENING: Duration = Duration::from_secs(60);

const CALLING: Duration = Duration::from_secs(300);

const FETCHING: Duration = Duration::from_secs(60);

const COMPLETING: Duration = Duration::from_secs(10);

const OVERHEARD: usize = 12;
const OVERHEARD_WIDTH: usize = 300;

pub(crate) struct Session {
    lister: Lister,
    connection: Connection,
    peer: Peer,
    completes: bool,
    reads: bool,
}

pub(crate) struct Ground<'a> {
    pub(crate) folder: Option<&'a Path>,
    pub(crate) vault: Option<&'a Vault>,
}

impl Session {
    pub(crate) async fn open(
        server: &Server,
        label: &str,
        ground: &Ground<'_>,
        watch: &Watch,
    ) -> Result<Session, Condition> {
        let lister = Lister::new(server.name(), label);
        let overheard = Overheard::new();

        let (link, arrivals) = match server.transport() {
            Transport::Stdio(stdio) => {
                let (link, arrivals, stderr) =
                    Link::stdio(command(stdio, ground.folder)).map_err(|error| {
                        failed(format!("{} could not be started: {error}", stdio.command()))
                    })?;
                if let Some(stderr) = stderr {
                    overheard.listen(label.to_owned(), stderr);
                }
                (link, arrivals)
            }
            Transport::Http(http) => {
                let signing = match ground.vault {
                    Some(vault) => Signing::stored(vault, http.url()).await,
                    None => None,
                };
                Link::http(http.url(), headers(http), signing).map_err(|error| {
                    failed(format!(
                        "{} is not an address to reach: {error}",
                        http.url()
                    ))
                })?
            }
        };

        let (connection, notes) = Connection::open(link, arrivals);
        tokio::spawn(Watcher::new(lister.clone(), watch).follow(connection.downgrade(), notes));

        let greeted = match tokio::time::timeout(OPENING, connection.initialize()).await {
            Ok(Ok(greeted)) => greeted,
            Ok(Err(failure)) if is_locked(&failure) => {
                return Err(Condition::Locked {
                    challenge: challenge(failure),
                });
            }
            Ok(Err(failure)) => return Err(failed(overheard.blamed(&chained(&failure)))),
            Err(_) => {
                return Err(failed(
                    overheard.blamed(&format!("it did not answer within {OPENING:?}")),
                ));
            }
        };

        let peer = described(&greeted);
        lister
            .everything(&connection, &greeted.capabilities)
            .await
            .map_err(failed)?;

        let session = Session {
            completes: greeted.capabilities.offers_completions(),
            reads: greeted.capabilities.offers_resources(),
            connection,
            lister,
            peer,
        };
        overheard.forget();
        Ok(session)
    }

    pub(crate) fn listing(&self) -> Listing {
        self.lister.listed().lock().clone()
    }

    pub(crate) fn listed(&self) -> Listed {
        Arc::clone(self.lister.listed())
    }

    pub(crate) fn peer(&self) -> &Peer {
        &self.peer
    }

    pub(crate) fn server(&self) -> &str {
        self.lister.server()
    }

    pub(crate) fn reads(&self) -> bool {
        self.reads
    }

    pub(crate) fn functions(&self) -> Vec<Function> {
        self.declared()
            .iter()
            .map(|declared| {
                let connection = self.connection.clone();
                let called = declared.called().to_owned();
                Function::new(declared.tool().clone(), move |input: Value| {
                    let connection = connection.clone();
                    let called = called.clone();
                    async move { call(&connection, called, input).await }
                })
            })
            .collect()
    }

    pub(crate) async fn prompted(
        &self,
        prompt: &str,
        arguments: Map<String, Value>,
    ) -> Result<Prompted, String> {
        let params = GetPromptParams {
            name: prompt.to_owned(),
            arguments: (!arguments.is_empty()).then_some(arguments),
        };
        let result: GetPromptResult = asked(
            FETCHING,
            self.connection.request(method::PROMPTS_GET, params),
        )
        .await?;
        Ok(Prompted::from_result(result))
    }

    pub(crate) async fn read(&self, uri: &str) -> Result<Vec<Contents>, String> {
        let params = ReadResourceParams {
            uri: uri.to_owned(),
        };
        let result: ReadResourceResult = asked(
            FETCHING,
            self.connection.request(method::RESOURCES_READ, params),
        )
        .await?;
        result
            .contents
            .into_iter()
            .map(Contents::from_resource)
            .collect()
    }

    pub(crate) async fn complete(
        &self,
        prompt: &str,
        argument: &str,
        typed: &str,
        given: HashMap<String, String>,
    ) -> Result<Vec<String>, String> {
        if !self.completes {
            return Ok(Vec::new());
        }
        let params = CompleteParams::prompt_argument(prompt, argument, typed, given);
        let result: CompleteResult = asked(
            COMPLETING,
            self.connection.request(method::COMPLETION_COMPLETE, params),
        )
        .await?;
        Ok(result.completion.values)
    }

    fn declared(&self) -> Arc<[Declared]> {
        Arc::clone(&self.lister.listed().lock().tools)
    }
}

fn failed(trouble: String) -> Condition {
    Condition::Unreached { trouble }
}

fn described(greeted: &InitializeResult) -> Peer {
    Peer {
        title: greeted.server_info.as_ref().map(|implementation| {
            let named = implementation
                .title
                .as_deref()
                .unwrap_or(&implementation.name);
            match implementation.description.as_deref().map(str::trim) {
                Some(description) if !description.is_empty() => {
                    format!("{named}: {description}")
                }
                _ => named.to_owned(),
            }
        }),
        instructions: greeted
            .instructions
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(Arc::from),
    }
}

fn is_locked(failure: &Failure) -> bool {
    matches!(
        failure,
        Failure::Link(LinkError::Unauthorized { .. } | LinkError::Signing(SigningError::SignedOut))
    )
}

fn challenge(failure: Failure) -> Option<String> {
    match failure {
        Failure::Link(LinkError::Unauthorized { challenge }) => challenge,
        _ => None,
    }
}

fn chained(error: &(dyn std::error::Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut deeper = error.source();
    while let Some(under) = deeper {
        message = format!("{message}: {under}");
        deeper = under.source();
    }
    message
}

async fn asked<T>(
    within: Duration,
    asking: impl Future<Output = Result<T, Failure>>,
) -> Result<T, String> {
    match tokio::time::timeout(within, asking).await {
        Err(_) => Err(format!("the server did not answer within {within:?}")),
        Ok(Err(failure)) if is_locked(&failure) => Err(String::from(
            "the server wants the person to sign in again before it answers",
        )),
        Ok(Err(Failure::Refused(error))) => Err(format!("the server refused: {}", error.message)),
        Ok(Err(failure)) => Err(format!(
            "the server could not be asked: {}",
            chained(&failure)
        )),
        Ok(Ok(answer)) => Ok(answer),
    }
}

async fn call(connection: &Connection, tool: String, input: Value) -> Result<Reply, String> {
    let arguments = match input {
        Value::Object(arguments) => Some(arguments),
        Value::Null => None,
        other => return Err(format!("the call is not an object: {other}")),
    };

    let params = CallToolParams {
        name: tool,
        arguments,
    };
    let result: CallToolResult =
        asked(CALLING, connection.request(method::TOOLS_CALL, params)).await?;
    outcome::replied(result)
}

fn command(stdio: &Stdio, project: Option<&Path>) -> Command {
    let mut command = Command::new(stdio.command());
    command.args(stdio.arguments());
    command.envs(stdio.variables());
    let folder = match (stdio.folder(), project) {
        (Some(folder), Some(project)) => Some(project.join(folder)),
        (Some(folder), None) => Some(folder.to_path_buf()),
        (None, project) => project.map(Path::to_path_buf),
    };
    if let Some(folder) = folder {
        command.current_dir(folder);
    }
    command
}

fn headers(http: &Http) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in http.headers() {
        let (Ok(name), Ok(mut value)) = (
            HeaderName::try_from(name.as_str()),
            HeaderValue::try_from(value.as_str()),
        ) else {
            tracing::warn!("skipping invalid header \"{name}\"");
            continue;
        };
        value.set_sensitive(name == header::AUTHORIZATION);
        headers.insert(name, value);
    }
    headers
}

#[derive(Clone)]
struct Overheard {
    stderr_lines: Arc<Mutex<Option<VecDeque<String>>>>,
}

impl Overheard {
    fn new() -> Overheard {
        Overheard {
            stderr_lines: Arc::new(Mutex::new(Some(VecDeque::new()))),
        }
    }

    fn listen(&self, label: String, stderr: ChildStderr) {
        let buffer = Arc::clone(&self.stderr_lines);
        tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("{label}: {line}");
                let mut captured = buffer.lock();
                let Some(captured) = captured.as_mut() else {
                    continue;
                };
                if captured.len() == OVERHEARD {
                    captured.pop_front();
                }
                captured.push_back(line.chars().take(OVERHEARD_WIDTH).collect());
            }
        });
    }

    fn forget(&self) {
        *self.stderr_lines.lock() = None;
    }

    fn blamed(&self, reason: &str) -> String {
        let lines = self
            .stderr_lines
            .lock()
            .take()
            .map(|lines| Vec::from(lines).join("\n"))
            .unwrap_or_default();

        match lines.is_empty() {
            true => format!("it could not be reached: {reason}"),
            false => format!("it could not be reached: {reason}\n{lines}"),
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("label", &self.lister.label())
            .field("tools", &self.lister.listed().lock().tools.len())
            .finish_non_exhaustive()
    }
}
