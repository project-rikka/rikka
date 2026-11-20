use std::time::Duration;

use rikka_seam::net::SocketClient;
use rikka_session::activation::{ActivationClient, ActivationToken};

const TOKEN: &str = "XDG_ACTIVATION_TOKEN";
const STARTUP_ID: &str = "DESKTOP_STARTUP_ID";

const TOKEN_TIMEOUT: Duration = Duration::from_millis(500);

pub(crate) fn arriving_token() -> Option<String> {
    let token = std::env::var(TOKEN)
        .ok()
        .or_else(|| std::env::var(STARTUP_ID).ok());
    unsafe {
        std::env::remove_var(TOKEN);
        std::env::remove_var(STARTUP_ID);
    }
    token.filter(|token| !token.is_empty())
}

pub(crate) fn token_for(app_id: &str, incoming: Option<String>) -> Option<String> {
    incoming.or_else(|| request_activation_token(app_id))
}

fn request_activation_token(app_id: &str) -> Option<String> {
    let (answer, reply) = std::sync::mpsc::channel();
    let app_id = app_id.to_owned();
    std::thread::Builder::new()
        .name("rikka-app-activation".to_owned())
        .spawn(move || {
            let token = mint_token(app_id)
                .inspect_err(|error| tracing::debug!(%error, "failed to mint activation token"))
                .ok();
            drop(answer.send(token));
        })
        .inspect_err(|error| tracing::debug!("failed to request activation token: {error}"))
        .ok()?;
    reply
        .recv_timeout(TOKEN_TIMEOUT)
        .inspect_err(|_| tracing::debug!("activation token request timed out"))
        .ok()
        .flatten()
        .filter(|token| !token.is_empty())
}

fn mint_token(app_id: String) -> Result<String, Box<dyn std::error::Error>> {
    let connection = SocketClient::from_env()?;
    let reply = ActivationClient::new(connection.client())
        .mint(app_id)
        .blocking_wait()?;
    Ok(reply.decode::<ActivationToken>()?.token)
}
