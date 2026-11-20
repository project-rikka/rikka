use rikka_llm_accounts::{AccountsChanged, Llm};
use rikka_widgets::column;

use crate::accounts::{self, Accounts};
use crate::language::{self, Language};
use crate::notice::notice;
use crate::prelude::*;
use crate::usage::Usage;

const FOLLOW: &str = "llm-settings.follow";

#[derive(Debug, State)]
pub struct LlmSettings {
    pub(crate) llm: Llm,
    pub(crate) accounts: Accounts,
    pub(crate) usage: Usage,
    pub(crate) language: Language,
}

impl LlmSettings {
    #[must_use]
    pub fn load() -> LlmSettings {
        let llm = Llm::load();
        LlmSettings {
            accounts: Accounts::new(&llm),
            language: Language::new(&llm),
            usage: Usage::default(),
            llm,
        }
    }

    pub fn open(&mut self) {
        self.llm.reload();
        self.accounts.reread(&self.llm);
        self.language.reread(&self.llm);
        self.usage.reread(&self.llm);
        if let Some(changes) = self.llm.changes() {
            stream_keyed(
                FOLLOW,
                changes,
                |settings: &mut LlmSettings, announced: AccountsChanged| {
                    settings.llm.apply_change(announced);
                },
            );
        }
    }

    pub fn close(&mut self) {
        cancel(FOLLOW);
        self.accounts.commit_all(&mut self.llm);
        self.language.commit(&mut self.llm);
    }
}

pub fn llm_settings(settings: &LlmSettings) -> impl View {
    column! {
        settings.llm.unavailable().map(|reason| {
            notice(
                banner(tr!("llm-settings.unavailable", reason = reason))
                    .symbol(MaterialSymbol::CLOUD_OFF)
                    .tint(palette().warning),
            )
        });
        settings.accounts
            .error()
            .map(|message| notice(banner(message).tint(palette().warning)));
        accounts::view(&settings.accounts, &settings.llm, &settings.usage);
        language::view(&settings.language);
    }
    .gap(Space::ROOMY)
    .fill_width()
}
