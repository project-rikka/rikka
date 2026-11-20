use std::sync::Arc;

use discord_client::{Link, Method, Prompt, SignInStep};
use discord_core::{Error, SavedAccount};
use rikka_account_book::PendingAccount;
use rikka_app::prelude::*;

use super::Chat;
use crate::signing::Signing;

const SIGN_IN: &str = "discord-sign-in";

impl Chat {
    pub(crate) fn add_account(&mut self) {
        self.sidebar.close_menu();
        self.sign_in_again();
    }

    pub(crate) fn sign_in_again(&mut self) {
        self.sign_in_with(Method::QrCode);
    }

    pub(crate) fn sign_in_with(&mut self, method: Method) {
        cancel(SIGN_IN);
        self.signing.open();
        let pending = match self.accounts.reserve() {
            Ok(pending) => pending,
            Err(error) => {
                self.signing.fail(error.to_string());
                return;
            }
        };
        let attempt = self.signing.begin(method);
        let sender = self.sender.clone();
        let prompt: Prompt = Arc::new(move |step: SignInStep| {
            let _ = sender.send_to(move |signing: &mut Signing| signing.prompted(attempt, step));
        });
        let discord = Arc::clone(&self.discord);
        let folder = pending.folder().to_owned();
        spawn_keyed(
            SIGN_IN,
            async move {
                let signed = discord.sign_in(&folder, method, prompt).await;
                (pending, signed)
            },
            move |chat: &mut Chat,
                  (pending, signed): (PendingAccount, discord_core::Result<Link>)| {
                chat.signed(attempt, pending, signed);
            },
        );
    }

    pub(crate) fn leave_sign_in(&mut self) {
        cancel(SIGN_IN);
        self.signing.close();
    }

    pub(crate) fn signed(
        &mut self,
        attempt: u64,
        pending: PendingAccount,
        signed: discord_core::Result<Link>,
    ) {
        if !self.signing.is_attempt(attempt) {
            return;
        }
        let link = match signed {
            Ok(link) => link,
            Err(Error::Cancelled) => return,
            Err(error) => {
                self.signing.fail(error.to_string());
                return;
            }
        };
        let account = link.account.clone();
        match self
            .accounts
            .commit(pending, |key| SavedAccount { key, account })
        {
            Ok(committed) => {
                self.signing.close();
                self.connected(committed.saved.key, link);
            }
            Err(error) => self.signing.fail(error.to_string()),
        }
    }
}
