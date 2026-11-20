use std::sync::mpsc::Sender;

use rikka_seam::SeamClient;
use rikka_settings::seam::{Announcement, Settings};
use rikka_settings::session::language;

use crate::controller::Message;

pub(crate) struct Language {
    settings: Option<Settings>,
}

impl Language {
    pub(crate) fn follow(
        seam: SeamClient,
        runtime: &tokio::runtime::Handle,
        messages: Sender<Message>,
    ) -> Language {
        if let Err(error) = rikka_i18n::embedded!().install() {
            tracing::warn!("cannot load translations: {error}");
        }
        let settings = Settings::mirror(language::schema(), seam)
            .inspect_err(|error| tracing::warn!("cannot follow the session language: {error}"))
            .ok();
        if let Some(changes) = settings.as_ref().and_then(Settings::changes) {
            runtime.spawn(crate::forward(changes, messages, Message::Language));
        }
        let language = Language { settings };
        language.apply();
        language
    }

    pub(crate) fn changed(&mut self, announcement: Announcement) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        match announcement {
            Announcement::Changed(changed) => {
                if !settings.apply_change(&changed) {
                    return;
                }
            }
            Announcement::Lagged => {
                if let Err(error) = settings.refresh() {
                    tracing::warn!("cannot reload the session language: {error}");
                }
            }
        }
        self.apply();
    }

    fn apply(&self) {
        let Some(tag) = self
            .settings
            .as_ref()
            .and_then(|settings| language::selected(settings.store()))
        else {
            return;
        };
        if rikka_i18n::set_language(&tag).is_none() {
            tracing::debug!("no catalog for `{tag}`");
        }
    }
}
