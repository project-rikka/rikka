use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use rikka_net::Url;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{Error, Result};

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const REQUEST_LINE_TIMEOUT: Duration = Duration::from_secs(5);

#[must_use]
#[derive(Debug, Clone)]
pub struct Loopback {
    host: &'static str,
    port: u16,
    path: String,
}

#[derive(Debug)]
pub struct Redirect {
    listeners: Vec<TcpListener>,
    path: String,
    uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pairs: Vec<(String, String)>,
}

impl Loopback {
    pub fn localhost(port: u16) -> Loopback {
        Loopback {
            host: "localhost",
            port,
            path: "/".to_owned(),
        }
    }

    pub fn ip(port: u16) -> Loopback {
        Loopback {
            host: "127.0.0.1",
            port,
            path: "/".to_owned(),
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Loopback {
        self.path = path.into();
        self
    }

    pub async fn bind(self) -> Result<Redirect> {
        let ipv4 = listen(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port).await?;
        let port = ipv4
            .local_addr()
            .map_err(|source| Error::Listen {
                address: format!("{}:{}", self.host, self.port),
                source,
            })?
            .port();

        let mut listeners = vec![ipv4];
        if self.host == "localhost"
            && let Ok(ipv6) = listen(IpAddr::V6(Ipv6Addr::LOCALHOST), port).await
        {
            listeners.push(ipv6);
        }

        Ok(Redirect {
            listeners,
            uri: format!("http://{}:{port}{}", self.host, self.path),
            path: self.path,
        })
    }
}

async fn listen(address: IpAddr, port: u16) -> Result<TcpListener> {
    TcpListener::bind((address, port))
        .await
        .map_err(|source| Error::Listen {
            address: format!("{address}:{port}"),
            source,
        })
}

impl Redirect {
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub async fn wait(&self) -> Result<Callback> {
        self.receive(None).await
    }

    pub async fn callback(&self, state: &str) -> Result<Callback> {
        self.receive(Some(state)).await
    }

    pub async fn code(&self, state: &str) -> Result<String> {
        self.callback(state).await?.code()
    }

    async fn receive(&self, state: Option<&str>) -> Result<Callback> {
        tokio::time::timeout(CALLBACK_TIMEOUT, self.serve(state))
            .await
            .map_err(|_| Error::TimedOut)?
    }

    async fn serve(&self, state: Option<&str>) -> Result<Callback> {
        loop {
            let stream = self.accept().await?;
            let Some((target, mut stream)) = request_target(stream).await else {
                continue;
            };

            let Some(callback) = self.match_callback(&target) else {
                respond(&mut stream, "404 Not Found", NOT_FOUND_PAGE).await;
                continue;
            };

            if state.is_some_and(|state| callback.has_state_mismatch(state)) {
                respond(&mut stream, "400 Bad Request", STATE_MISMATCH_PAGE).await;
                continue;
            }

            let page = match callback.get("error") {
                None => SUCCESS_PAGE,
                Some(_) => FAILURE_PAGE,
            };
            respond(&mut stream, "200 OK", page).await;
            return Ok(callback);
        }
    }

    async fn accept(&self) -> Result<TcpStream> {
        let pending = self
            .listeners
            .iter()
            .map(|listener| Box::pin(listener.accept()));

        let (accepted, _, _) = futures_util::future::select_all(pending).await;
        accepted
            .map(|(stream, _)| stream)
            .map_err(|source| Error::Listen {
                address: self.uri.clone(),
                source,
            })
    }

    fn match_callback(&self, target: &str) -> Option<Callback> {
        let url = Url::parse(&format!("http://localhost{target}")).ok()?;
        let expected_path = self.path.trim_end_matches('/');
        (url.path().trim_end_matches('/') == expected_path).then(|| Callback {
            pairs: url.query_pairs().into_owned().collect(),
        })
    }
}

impl Callback {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(key, value)| key == name && !value.is_empty())
            .map(|(_, value)| value.as_str())
    }

    pub fn code(&self) -> Result<String> {
        match self.get("error") {
            Some("access_denied") => return Err(Error::AccessDenied),
            Some(error) => {
                return Err(Error::Failed(match self.get("error_description") {
                    Some(description) => format!("{error}: {description}"),
                    None => error.to_owned(),
                }));
            }
            None => {}
        }

        self.get("code")
            .map(str::to_owned)
            .ok_or(Error::MissingField("code"))
    }

    fn has_state_mismatch(&self, state: &str) -> bool {
        match self.get("state") {
            Some(received) => received != state,
            None => self.get("error").is_none(),
        }
    }
}

async fn request_target(stream: TcpStream) -> Option<(String, TcpStream)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    let read = tokio::time::timeout(REQUEST_LINE_TIMEOUT, reader.read_line(&mut line))
        .await
        .ok()?
        .ok()?;
    if read == 0 {
        return None;
    }

    let target = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    Some((target, reader.into_inner()))
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

const SUCCESS_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:100vh;margin:0\">\
<p>Signed in. You can close this window.</p>";

const FAILURE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Sign-in failed</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:100vh;margin:0\">\
<p>Sign-in failed. You can close this window.</p>";

const STATE_MISMATCH_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Another sign-in</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:100vh;margin:0\">\
<p>This page belongs to another sign-in.</p>";

const NOT_FOUND_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Not found</title>";
