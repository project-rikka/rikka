use crate::error::{Error, Result};
use crate::signing::{Prompt, SignInStep, WebSignIn, answered};

const LOGIN_PAGE: &str = "https://discord.com/login";
const MESSAGE: &str = "rikkaDiscordToken";
const SCRIPT: &str = "discord-client/token.js";

pub(crate) async fn sign_in(prompt: Prompt) -> Result<String> {
    let (request, answer) = WebSignIn::new(LOGIN_PAGE, SCRIPT, MESSAGE);
    prompt(SignInStep::Web(request));
    let posted = answered(answer).await?;
    let token = posted.trim().trim_matches('"').to_owned();
    if token.is_empty() {
        return Err(Error::Cancelled);
    }
    Ok(token)
}
