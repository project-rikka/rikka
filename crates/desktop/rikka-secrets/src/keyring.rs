use std::future::Future;

use rikka_secrets_protocol::WireSecret;
use rikka_secrets_protocol::names::{KEYRING_INTERFACE, PROMPT_INTERFACE, TEXT_CONTENT};
use zbus::Proxy;
use zbus::zvariant::OwnedObjectPath;

use crate::error::{Error, Result};
use crate::secrets::{Secrets, service_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyringState {
    Absent,
    Locked,
    Refused,
    Unlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOutcome {
    Created,
    Unlocked,
    Changed,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PromptId(OwnedObjectPath);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub state: KeyringState,
    pub auto_unlock: bool,
    pub asking: Vec<PromptId>,
}

#[derive(Clone, Debug)]
pub struct Keyring {
    secrets: Secrets,
}

impl Keyring {
    pub(crate) fn new(secrets: Secrets) -> Keyring {
        Keyring { secrets }
    }

    pub async fn status(&self) -> Result<Status> {
        let keyring = self.proxy().await?;
        let state: String = keyring
            .get_property("State")
            .await
            .map_err(Error::from_call)?;
        let auto_unlock: bool = keyring
            .get_property("AutoUnlock")
            .await
            .map_err(Error::from_call)?;
        let asking: Vec<OwnedObjectPath> = keyring
            .get_property("Asking")
            .await
            .map_err(Error::from_call)?;
        let state = match state.as_str() {
            "absent" => KeyringState::Absent,
            "locked" => KeyringState::Locked,
            "refused" => KeyringState::Refused,
            "unlocked" => KeyringState::Unlocked,
            other => return Err(Error::Service(format!("unknown keyring state {other}"))),
        };
        Ok(Status {
            state,
            auto_unlock,
            asking: asking.into_iter().map(PromptId).collect(),
        })
    }

    pub async fn login(&self, password: &[u8]) -> Result<LoginOutcome> {
        let keyring = self.proxy().await?;
        let outcome: String = self
            .sealed(password, |sealed| {
                let keyring = keyring.clone();
                async move { keyring.call("Login", &(sealed,)).await }
            })
            .await?;
        match outcome.as_str() {
            "created" => Ok(LoginOutcome::Created),
            "unlocked" => Ok(LoginOutcome::Unlocked),
            "changed" => Ok(LoginOutcome::Changed),
            "refused" => Ok(LoginOutcome::Refused),
            other => Err(Error::Service(format!("unknown login outcome {other}"))),
        }
    }

    pub async fn answer(&self, prompt: &PromptId, password: &[u8]) -> Result<bool> {
        let keyring = self.proxy().await?;
        self.sealed(password, |sealed| {
            let (keyring, prompt) = (keyring.clone(), prompt.0.clone());
            async move { keyring.call("Answer", &(prompt, sealed)).await }
        })
        .await
    }

    pub async fn dismiss(&self, prompt: &PromptId) -> Result<()> {
        self.secrets
            .proxy(&prompt.0, PROMPT_INTERFACE)
            .await?
            .call::<_, _, ()>("Dismiss", &())
            .await
            .map_err(Error::from_call)
    }

    pub async fn reset(&self) -> Result<()> {
        self.proxy()
            .await?
            .call::<_, _, ()>("Reset", &())
            .await
            .map_err(Error::from_call)
    }

    pub async fn set_auto_unlock(&self, enabled: bool) -> Result<()> {
        self.proxy()
            .await?
            .call::<_, _, ()>("SetAutoUnlock", &(enabled,))
            .await
            .map_err(Error::from_call)
    }

    async fn proxy(&self) -> Result<Proxy<'static>> {
        self.secrets.proxy(&service_path(), KEYRING_INTERFACE).await
    }

    async fn sealed<R, Call, Reply>(&self, password: &[u8], call: Call) -> Result<R>
    where
        Call: Fn(WireSecret) -> Reply,
        Reply: Future<Output = zbus::Result<R>>,
    {
        self.secrets
            .with_session(|session| Ok(call(session.seal(password, TEXT_CONTENT)?)))
            .await
            .map(|(_, reply)| reply)
    }
}
