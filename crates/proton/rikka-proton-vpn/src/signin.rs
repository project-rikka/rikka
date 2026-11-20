use std::time::Duration;

use jiff::{SignedDuration, Timestamp};
use rikka_proton_account::{Account, ForkInvitation, SessionInfo};
use serde::Deserialize;

use crate::error::{Error, Result};

const CODE_URL: &str = "https://account.proton.me/vpn/tv/code";
const CODE_LIFETIME: SignedDuration = SignedDuration::from_mins(10);
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const PENDING_STATUS: u16 = 422;
const WEB_CHILD_CLIENT_ID: &str = "web-account-lite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInCode {
    selector: String,
    user_code: String,
    expires_at: Timestamp,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SignInPayload {
    #[serde(rename = "InitialUserTier")]
    pub initial_user_tier: String,
    #[serde(rename = "FlowType")]
    pub flow_type: String,
}

#[derive(Debug, Clone)]
pub struct SignedIn {
    pub session: SessionInfo,
    pub payload: Option<SignInPayload>,
}

impl SignInCode {
    pub(crate) fn new(invitation: &ForkInvitation, now: Timestamp) -> SignInCode {
        SignInCode {
            selector: invitation.selector().to_owned(),
            user_code: invitation.user_code().to_owned(),
            expires_at: now.saturating_add(CODE_LIFETIME).unwrap_or(Timestamp::MAX),
        }
    }

    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    #[must_use]
    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    #[must_use]
    pub fn url(&self, client_id: &str) -> String {
        format!(
            "{CODE_URL}/{}?clientId={}",
            rikka_proton_api::percent::encode(&self.user_code),
            rikka_proton_api::percent::encode(client_id)
        )
    }
}

pub(crate) async fn begin(account: &Account) -> Result<SignInCode> {
    account.ensure_session().await?;
    let invitation = account.fork_invitation().await?;
    Ok(SignInCode::new(&invitation, Timestamp::now()))
}

pub(crate) async fn poll(account: &Account, code: &SignInCode) -> Result<Option<SignedIn>> {
    match account.forked_session(code.selector()).await {
        Ok(forked) => {
            let payload = forked.sealed_payload().and_then(read_payload);
            let session = forked.into_session();
            account.api().set_session(session.session())?;
            Ok(Some(SignedIn { session, payload }))
        }
        Err(rikka_proton_account::Error::Api(error)) if is_pending(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn wait(account: &Account, code: &SignInCode) -> Result<SignedIn> {
    loop {
        if Timestamp::now() >= code.expires_at() {
            return Err(Error::SignInExpired);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
        if let Some(signed_in) = poll(account, code).await? {
            return Ok(signed_in);
        }
    }
}

pub(crate) async fn web_url(account: &Account, url: &str) -> Result<String> {
    let selector = account
        .fork()
        .child_client_id(WEB_CHILD_CLIENT_ID)
        .payload("")
        .independent(false)
        .execute()
        .await?;
    let base = url.split_once('#').map_or(url, |(base, _)| base);
    Ok(format!("{base}#selector={selector}"))
}

fn is_pending(error: &rikka_proton_api::Error) -> bool {
    error
        .status()
        .is_some_and(|status| status.as_u16() == PENDING_STATUS)
        || matches!(
            error,
            rikka_proton_api::Error::Network(_) | rikka_proton_api::Error::NoRoute
        )
}

fn read_payload(sealed: &str) -> Option<SignInPayload> {
    let bytes = rikka_proton_api::base64::decode(sealed).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(payload) => Some(payload),
        Err(error) => {
            tracing::warn!(%error, "failed to parse sign-in payload");
            None
        }
    }
}
