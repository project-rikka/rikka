use futures_util::Stream;
use rikka_llm::CancelToken;
use rikka_llm_providers::{Backend, Credential, Login, Prompt};

use crate::error::Error;

#[derive(Clone, Debug)]
pub enum Step {
    Prompt(Backend, Prompt),
    Done(Backend, Credential),
    Failed(Backend, String),
}

pub fn sign_in(backend: Backend, cancel: CancelToken) -> impl Stream<Item = Step> + Send {
    futures_util::stream::unfold(Stage::Start(backend, cancel), |stage| async move {
        match stage {
            Stage::Start(backend, cancel) => Some(start(backend, cancel).await),
            Stage::Waiting(backend, login, cancel) => {
                Some((finish(backend, login, cancel).await, Stage::Ended))
            }
            Stage::Ended => None,
        }
    })
}

enum Stage {
    Start(Backend, CancelToken),
    Waiting(Backend, Login, CancelToken),
    Ended,
}

async fn start(backend: Backend, cancel: CancelToken) -> (Step, Stage) {
    let outcome = tokio::select! {
        biased;
        () = cancel.cancelled() => return (gave_up(backend), Stage::Ended),
        outcome = backend.login() => outcome,
    };

    let login = match outcome {
        Ok(login) => login,
        Err(refused) => return (Step::Failed(backend, refused.to_string()), Stage::Ended),
    };
    if cancel.is_cancelled() {
        return (gave_up(backend), Stage::Ended);
    }

    let prompt = login.prompt().clone();
    if let Err(error) = rikka_open::open(prompt.url()) {
        tracing::warn!("failed to open sign-in page: {error}");
    }
    (
        Step::Prompt(backend, prompt),
        Stage::Waiting(backend, login, cancel),
    )
}

async fn finish(backend: Backend, login: Login, cancel: CancelToken) -> Step {
    tokio::select! {
        biased;
        () = cancel.cancelled() => gave_up(backend),
        redeemed = login.finish() => match redeemed {
            Ok(credential) => Step::Done(backend, credential),
            Err(refused) => Step::Failed(backend, refused.to_string()),
        },
    }
}

fn gave_up(backend: Backend) -> Step {
    Step::Failed(backend, Error::Stopped.to_string())
}
