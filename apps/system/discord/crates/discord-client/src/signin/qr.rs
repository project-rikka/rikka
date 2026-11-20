use std::sync::Arc;
use std::time::Duration;

use rikka_net::{Message, header};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::Instant;

use super::keys::Keys;
use crate::error::{Error, Result};
use crate::identity::{Identity, ORIGIN, USER_AGENT};
use crate::rest::{Rest, Token};
use crate::signing::{Prompt, QrStep, SignInStep};

const GATEWAY: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
const CODE_URL: &str = "https://discord.com/ra/";
const LOGIN: &str = "/users/@me/remote-auth/login";
const MAX_CODES: u32 = 20;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Inbound {
    op: String,
    heartbeat_interval: Option<u64>,
    encrypted_nonce: Option<String>,
    fingerprint: Option<String>,
    encrypted_user_payload: Option<String>,
    ticket: Option<String>,
}

#[derive(Deserialize)]
struct Login {
    encrypted_token: String,
}

enum Round {
    Token(String),
    Again,
}

pub(crate) async fn sign_in(prompt: Prompt, identity: Arc<Identity>) -> Result<String> {
    let rest = Rest::new(Arc::new(Token::new("")), identity)?;
    for _ in 0..MAX_CODES {
        let keys = tokio::task::spawn_blocking(Keys::generate)
            .await
            .map_err(|_| Error::Cancelled)??;
        match round(&keys, &prompt, &rest).await? {
            Round::Token(token) => return Ok(token),
            Round::Again => continue,
        }
    }
    Err(Error::Handshake("the code expired too many times".into()))
}

async fn round(keys: &Keys, prompt: &Prompt, rest: &Rest) -> Result<Round> {
    let mut socket = rest
        .http()
        .websocket(GATEWAY)
        .header(header::ORIGIN, ORIGIN)
        .header(header::USER_AGENT, USER_AGENT)
        .no_heartbeat()
        .connect()
        .await
        .map_err(Error::unreachable)?;
    let mut every: Option<Duration> = None;
    let mut next = far_future();
    let mut scanned = false;
    loop {
        tokio::select! {
            received = socket.recv() => {
                let text = match received {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => {
                        return if scanned {
                            Err(Error::Handshake("the phone closed the sign-in".into()))
                        } else {
                            Ok(Round::Again)
                        };
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => return Err(Error::unreachable(error)),
                };
                let inbound: Inbound = serde_json::from_str(&text).unwrap_or_default();
                match inbound.op.as_str() {
                    "hello" => {
                        let interval = Duration::from_millis(inbound.heartbeat_interval.unwrap_or(41_250));
                        every = Some(interval);
                        next = Instant::now() + interval;
                        send(&mut socket, json!({ "op": "init", "encoded_public_key": keys.encoded_public_key() })).await?;
                    }
                    "nonce_proof" => {
                        let nonce = inbound.encrypted_nonce.ok_or_else(|| Error::Handshake("no nonce".into()))?;
                        send(&mut socket, json!({ "op": "nonce_proof", "nonce": keys.proof(&nonce)? })).await?;
                    }
                    "pending_remote_init" => {
                        let fingerprint = inbound.fingerprint.unwrap_or_default();
                        if fingerprint != keys.fingerprint() {
                            return Err(Error::Handshake("the fingerprint does not match our key".into()));
                        }
                        prompt(SignInStep::Qr(QrStep::Code(format!("{CODE_URL}{fingerprint}"))));
                    }
                    "pending_ticket" => {
                        let sealed = inbound.encrypted_user_payload.unwrap_or_default();
                        let payload = String::from_utf8(keys.decrypt(&sealed)?).map_err(Error::unreadable)?;
                        scanned = true;
                        prompt(SignInStep::Qr(scanned_step(&payload)));
                    }
                    "pending_login" => {
                        let ticket = inbound.ticket.ok_or_else(|| Error::Handshake("no ticket".into()))?;
                        let _ = socket.close().await;
                        let login: Login = rest.post(LOGIN).json(json!({ "ticket": ticket })).parse().await?;
                        let token = String::from_utf8(keys.decrypt(&login.encrypted_token)?).map_err(Error::unreadable)?;
                        return Ok(Round::Token(token));
                    }
                    "cancel" => return Ok(Round::Again),
                    _ => {}
                }
            }
            () = tokio::time::sleep_until(next), if every.is_some() => {
                next = Instant::now() + every.unwrap_or_default();
                send(&mut socket, json!({ "op": "heartbeat" })).await?;
            }
        }
    }
}

pub(crate) fn scanned_step(payload: &str) -> QrStep {
    let mut parts = payload.splitn(4, ':');
    let id = parts.next().unwrap_or_default();
    let discriminator = parts.next().filter(|tag| !tag.is_empty());
    let avatar = parts.next().filter(|hash| !hash.is_empty() && *hash != "0");
    let name = parts.next().unwrap_or_default().to_owned();
    QrStep::Scanned {
        name,
        avatar: (!id.is_empty()).then(|| discord_translate::avatar(id, avatar, discriminator)),
    }
}

async fn send(socket: &mut rikka_net::WebSocket, payload: Value) -> Result<()> {
    socket
        .send_text(payload.to_string())
        .await
        .map_err(Error::unreachable)
}

fn far_future() -> Instant {
    Instant::now() + Duration::from_secs(24 * 60 * 60)
}
