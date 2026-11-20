use music_core::{Error, Prompt, Result, SignInStep};
use rikka_net::Url;
use rikka_oauth::{Callback, Loopback};

use crate::bundle::App;

const SIGN_IN: &str = "https://www.qobuz.com/signin/oauth";
const ANY_PORT: u16 = 0;

pub(crate) async fn authorize(app: &App, prompt: Prompt) -> Result<String> {
    let redirect = Loopback::ip(ANY_PORT)
        .path(format!("/{}", rikka_oauth::random_state()))
        .bind()
        .await
        .map_err(failed)?;
    prompt(SignInStep::Browse(sign_in_page(app, redirect.uri())?));

    let callback = redirect.wait().await.map_err(failed)?;
    code(&callback)
}

fn sign_in_page(app: &App, redirect: &str) -> Result<String> {
    let mut url = Url::parse(SIGN_IN).map_err(|error| Error::Unreadable(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("ext_app_id", &app.id)
        .append_pair("redirect_url", redirect);
    Ok(url.into())
}

fn code(callback: &Callback) -> Result<String> {
    callback
        .get("code_autorisation")
        .or_else(|| callback.get("code"))
        .map(str::to_owned)
        .ok_or_else(|| Error::Refused("Qobuz returned no sign-in code".into()))
}

fn failed(error: rikka_oauth::Error) -> Error {
    match error {
        rikka_oauth::Error::AccessDenied | rikka_oauth::Error::TimedOut => Error::Cancelled,
        error => Error::Unreachable(error.to_string()),
    }
}
