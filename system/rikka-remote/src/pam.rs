use std::path::Path;
use std::time::Duration;

use rikka_pam::{Authentication, Outcome, Secret, Step};

const SERVICE: &str = "rikka-remote";
const FALLBACK_SERVICE: &str = "login";
const PAM_CONFIG: &str = "/etc/pam.d";
const PATIENCE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    Granted,
    Denied(String),
    Unavailable(String),
}

pub(crate) fn service() -> &'static str {
    match Path::new(PAM_CONFIG).join(SERVICE).exists() {
        true => SERVICE,
        false => FALLBACK_SERVICE,
    }
}

pub(crate) async fn verify(user: &str, password: String) -> Verdict {
    let (steps, mut received) = tokio::sync::mpsc::unbounded_channel();
    let authentication = match Authentication::start(service(), user, move |step| {
        if steps.send(step).is_err() {
            tracing::debug!("PAM step after the login was abandoned");
        }
    }) {
        Ok(authentication) => authentication,
        Err(error) => return Verdict::Unavailable(error.to_string()),
    };
    let mut password = Some(Secret::new(password));
    let conversation = async {
        while let Some(step) = received.recv().await {
            match step {
                Step::Prompt(prompt) if !prompt.echo => match password.take() {
                    Some(secret) => authentication.answer(prompt.serial, secret),
                    None => return Verdict::Denied(prompt.text),
                },
                Step::Prompt(prompt) => return Verdict::Denied(prompt.text),
                Step::Message(message) => {
                    tracing::debug!(text = message.text, "PAM message");
                }
                Step::Finished(Outcome::Granted) => return Verdict::Granted,
                Step::Finished(Outcome::Denied(reason)) => return Verdict::Denied(reason),
                Step::Finished(Outcome::Cancelled) => {
                    return Verdict::Denied("authentication was cancelled".into());
                }
                Step::Finished(Outcome::Error(reason)) => return Verdict::Unavailable(reason),
            }
        }
        Verdict::Unavailable("the PAM helper stopped".into())
    };
    match tokio::time::timeout(PATIENCE, conversation).await {
        Ok(verdict) => verdict,
        Err(_) => Verdict::Unavailable("authentication timed out".into()),
    }
}
