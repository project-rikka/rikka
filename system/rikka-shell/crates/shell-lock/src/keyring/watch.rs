use futures_util::StreamExt;
use rikka_secrets::{Change, KeyringState, Secrets};
use shell_kit::detached_future;
use zeroize::Zeroizing;

use crate::keyring::service::Report;

const WATCHING: &str = "rikka-keyring-watch";
const LOGGING_IN: &str = "rikka-keyring-login";
const ASKING: &str = "rikka-keyring-ask";
const BINDING: &str = "rikka-keyring-binding";

pub(crate) fn log_in(password: Zeroizing<String>) {
    detached(LOGGING_IN, async move {
        let secrets = Secrets::shared().await?;
        let outcome = secrets.keyring().login(password.as_bytes()).await?;
        tracing::info!(?outcome, "keyring answered the login password");
        Ok(())
    });
}

pub(super) fn set_auto_unlock(enabled: bool) {
    detached(BINDING, async move {
        Secrets::shared()
            .await?
            .keyring()
            .set_auto_unlock(enabled)
            .await
    });
}

pub(super) fn watch(report: impl Fn(Report) + Send + 'static) {
    detached(WATCHING, async move {
        let secrets = Secrets::shared().await?;
        let mut changes = secrets
            .changes()
            .await?
            .filter(|change| std::future::ready(*change == Change::Keyring));
        let keyring = secrets.keyring();
        let mut asked_after_refusal = false;
        loop {
            match keyring.status().await {
                Ok(status) => {
                    let refused = status.state == KeyringState::Refused;
                    if refused && status.asking.is_empty() && !asked_after_refusal {
                        ask_to_unlock(secrets.clone());
                    }
                    asked_after_refusal = refused;
                    report(Report {
                        asking: !status.asking.is_empty(),
                    });
                }
                Err(error) => tracing::warn!("failed to read the keyring status: {error}"),
            }
            if changes.next().await.is_none() {
                return Ok(());
            }
        }
    });
}

fn ask_to_unlock(secrets: Secrets) {
    detached(ASKING, async move {
        match secrets.unlock().await {
            Err(rikka_secrets::Error::Dismissed) => Ok(()),
            outcome => outcome,
        }
    });
}

fn detached(
    name: &'static str,
    work: impl Future<Output = rikka_secrets::Result<()>> + Send + 'static,
) {
    detached_future(name, async move {
        if let Err(error) = work.await {
            tracing::warn!("keyring request failed: {error}");
        }
    });
}
