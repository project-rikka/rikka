use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm_adapter::{Grant, Prompt};
use serde_json::Value;

use crate::backend::Backend;
use crate::credential::Credential;
use crate::error::{Error, Result};

pub struct Login {
    prompt: Prompt,
    redeeming: BoxFuture<'static, rikka_llm::Result<Credential>>,
}

impl Login {
    pub(crate) fn new(backend: Backend, login: rikka_llm_adapter::Login<Value>) -> Login {
        let (prompt, redeeming) = login.into_parts();
        let redeeming = redeeming.map(move |redeemed| {
            redeemed.map(|signed| match signed {
                Grant::Key(key) => Credential::keyed(key),
                Grant::Tokens(tokens) => Credential::signed(backend, tokens),
            })
        });
        Login {
            prompt,
            redeeming: redeeming.boxed(),
        }
    }

    #[must_use]
    pub fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    pub async fn finish(self) -> Result<Credential> {
        self.redeeming.await.map_err(Error::SignIn)
    }
}

impl std::fmt::Debug for Login {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Login")
            .field("prompt", &self.prompt)
            .finish_non_exhaustive()
    }
}
