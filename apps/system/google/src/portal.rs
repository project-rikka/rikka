use std::future::Future;

use google_account::{SIGN_IN_COOKIE, SIGN_IN_COOKIE_SITE, SIGN_IN_PAGE};
use google_passwords::{
    KEY_RETRIEVAL_MESSAGE, KEY_RETRIEVAL_PAGE, KEY_RETRIEVAL_SCRIPT, KEY_RETRIEVAL_SITE,
    posted_vault_keys,
};
use rikka_web::{
    InjectAt, NewSessionRequest, Script, ScriptMessage, Session, SignInPortal, UserScript, World,
};

const KEYS_HOOK: &str = "rikka-google-vault-keys";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    SignIn,
    VaultKeys,
}

#[derive(Debug)]
pub(crate) struct Portal {
    portal: SignInPortal,
    purpose: Purpose,
}

impl Portal {
    pub(crate) fn open(purpose: Purpose) -> Result<Portal, String> {
        let portal = match purpose {
            Purpose::SignIn => SignInPortal::open(SIGN_IN_PAGE, &[]),
            Purpose::VaultKeys => {
                let hook = UserScript::new(KEYS_HOOK, Script::bundled(KEY_RETRIEVAL_SCRIPT))
                    .inject_at(InjectAt::Start)
                    .world(World::Page)
                    .matches([KEY_RETRIEVAL_SITE]);
                SignInPortal::open(KEY_RETRIEVAL_PAGE, &[hook])
            }
        };
        Ok(Portal {
            portal: portal.map_err(|error| error.to_string())?,
            purpose,
        })
    }

    pub(crate) fn purpose(&self) -> Purpose {
        self.purpose
    }

    pub(crate) fn session(&self) -> &Session {
        self.portal.session()
    }

    pub(crate) fn follow(&self, opening: NewSessionRequest) {
        self.portal.follow(opening);
    }

    pub(crate) fn sign_in_token(
        &self,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send + use<> {
        let cookies = self.portal.cookies(SIGN_IN_COOKIE_SITE);
        async move {
            Ok(cookies
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|cookie| cookie.name == SIGN_IN_COOKIE && !cookie.value.is_empty())
                .map(|cookie| cookie.value))
        }
    }

    pub(crate) fn vault_keys(
        &self,
        message: &ScriptMessage,
        account: &str,
    ) -> Option<Vec<Vec<u8>>> {
        if message.name != KEY_RETRIEVAL_MESSAGE || message.world != World::Page {
            return None;
        }
        posted_vault_keys(&message.body, account)
    }
}
