use rikka_seam::{Request, SeamClient};
use rikka_secrets::{KeyringState, PromptId, Secrets, Status};
use rikka_session::following;
use rikka_ui::{State, TextDocument, spawn, stream};
use zeroize::Zeroizing;

use crate::keyring::service::{CAPABILITY, CHANGED, PENDING, Report};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Asking,
    Checking,
    Refused,
    Resetting,
    Broken,
}

#[derive(Debug, State)]
pub(super) struct Dialog {
    seam: SeamClient,
    status: Option<Status>,
    typed: TextDocument,
    phase: Phase,
}

impl Dialog {
    #[must_use]
    pub(super) fn new(seam: SeamClient) -> Dialog {
        Dialog {
            seam,
            status: None,
            typed: TextDocument::default(),
            phase: Phase::Asking,
        }
    }

    pub(super) fn start(&mut self) {
        let announced = following::states::<Report>(&self.seam, CAPABILITY, CHANGED, || {
            Request::new(CAPABILITY, PENDING)
        });
        stream(announced, |dialog: &mut Dialog, _: Report| dialog.refresh());
        self.refresh();
    }

    pub(super) fn state(&self) -> Option<KeyringState> {
        self.status.as_ref().map(|status| status.state)
    }

    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    pub(super) fn typed(&self) -> &TextDocument {
        &self.typed
    }

    pub(super) fn submit(&mut self) {
        let Some(prompt) = self.prompt() else {
            return;
        };
        let password = Zeroizing::new(self.typed.text());
        self.typed = TextDocument::default();
        if password.is_empty() {
            return;
        }
        self.phase = Phase::Checking;
        spawn(
            async move {
                Secrets::shared()
                    .await?
                    .keyring()
                    .answer(&prompt, password.as_bytes())
                    .await
            },
            |dialog: &mut Dialog, opened: rikka_secrets::Result<bool>| {
                dialog.phase = match opened {
                    Ok(true) => Phase::Asking,
                    Ok(false) => Phase::Refused,
                    Err(error) => {
                        tracing::warn!("failed to unlock the keyring: {error}");
                        Phase::Broken
                    }
                };
            },
        );
    }

    pub(super) fn dismiss(&mut self) {
        let Some(prompt) = self.prompt() else {
            return;
        };
        spawn(
            async move { Secrets::shared().await?.keyring().dismiss(&prompt).await },
            |dialog: &mut Dialog, dismissed: rikka_secrets::Result<()>| {
                if let Err(error) = dismissed {
                    tracing::warn!("failed to dismiss the keyring prompt: {error}");
                    dialog.phase = Phase::Broken;
                }
            },
        );
    }

    pub(super) fn ask_to_reset(&mut self) {
        self.phase = Phase::Resetting;
    }

    pub(super) fn keep(&mut self) {
        self.phase = Phase::Asking;
    }

    pub(super) fn reset(&mut self) {
        spawn(
            async move { Secrets::shared().await?.keyring().reset().await },
            |dialog: &mut Dialog, reset: rikka_secrets::Result<()>| {
                dialog.phase = match reset {
                    Ok(()) => Phase::Asking,
                    Err(error) => {
                        tracing::warn!("failed to reset the keyring: {error}");
                        Phase::Broken
                    }
                };
            },
        );
    }

    fn prompt(&self) -> Option<PromptId> {
        self.status
            .as_ref()
            .and_then(|status| status.asking.first().cloned())
    }

    fn refresh(&mut self) {
        spawn(
            async move { Secrets::shared().await?.keyring().status().await },
            |dialog: &mut Dialog, status: rikka_secrets::Result<Status>| match status {
                Ok(status) => {
                    if dialog.prompt().as_ref() != status.asking.first() {
                        dialog.typed = TextDocument::default();
                        dialog.phase = Phase::Asking;
                    }
                    dialog.status = Some(status);
                }
                Err(error) => tracing::warn!("failed to read the keyring status: {error}"),
            },
        );
    }
}
