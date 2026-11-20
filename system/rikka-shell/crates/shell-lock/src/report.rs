use rikka_pam::{MessageKind, Outcome, Step};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub(super) talk: u64,
    pub(crate) event: AuthEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AuthEvent {
    PamQuestion {
        serial: u64,
        text: String,
        echo: bool,
    },
    Message {
        error: bool,
        text: String,
    },
    Granted,
    Denied,
    Cancelled,
    Broken(String),
}

impl From<Step> for AuthEvent {
    fn from(step: Step) -> AuthEvent {
        match step {
            Step::Prompt(prompt) => AuthEvent::PamQuestion {
                serial: prompt.serial,
                text: prompt.text,
                echo: prompt.echo,
            },
            Step::Message(message) => AuthEvent::Message {
                error: message.kind == MessageKind::Error,
                text: message.text,
            },
            Step::Finished(Outcome::Granted) => AuthEvent::Granted,
            Step::Finished(Outcome::Denied(reason)) => {
                tracing::info!("screen stays locked: {reason}");
                AuthEvent::Denied
            }
            Step::Finished(Outcome::Cancelled) => AuthEvent::Cancelled,
            Step::Finished(Outcome::Error(reason)) => AuthEvent::Broken(reason),
        }
    }
}
