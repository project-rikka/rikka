use discord_core::{InviteCode, SpaceDraft, SpaceId, SpaceTemplate};
use rikka_app::prelude::*;

use super::Chat;
use crate::space_sheet::{self, SpaceSheet};

impl Chat {
    pub(crate) fn space_sheet(&self) -> &SpaceSheet {
        &self.space_sheet
    }

    pub(crate) fn open_space_sheet(&mut self) {
        let Some(linked) = self.current() else {
            return;
        };
        let default_name = linked.me().map_or_else(String::new, |me| {
            tr!("discord.space-sheet.default-name", name = me.name()).to_string()
        });
        self.close_overlays();
        self.space_sheet.open(&default_name);
    }

    pub(crate) fn close_space_sheet(&mut self) {
        self.space_sheet.close();
    }

    pub(crate) fn choose_space_icon(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || space_sheet::chosen(parent),
            |chat: &mut Chat, picked| chat.space_sheet.iconed(picked),
        );
    }

    pub(crate) fn create_space(&mut self, template: SpaceTemplate) {
        let name = self.space_sheet.name();
        if name.is_empty() || self.space_sheet.is_busy() {
            return;
        }
        let draft = SpaceDraft {
            name,
            png_icon: self.space_sheet.icon().cloned(),
            template,
        };
        self.add_space(
            move |client| async move { client.create_space(&draft).await },
            |error| error.to_string(),
        );
    }

    pub(crate) fn join_space(&mut self) {
        if self.space_sheet.is_busy() {
            return;
        }
        let Some(invite) = InviteCode::parse(&self.space_sheet.invite()) else {
            self.space_sheet
                .failed(tr!("discord.space-sheet.invalid-invite").to_string());
            return;
        };
        self.add_space(
            move |client| async move { client.join_space(&invite).await },
            join_refusal,
        );
    }

    pub(crate) fn space_arrived(&mut self, key: &str, space: &SpaceId) {
        if self.space_sheet.take_awaited(key, space) && self.current_key() == Some(key) {
            self.open_space(space);
        }
    }

    fn add_space<F>(
        &mut self,
        ask: impl FnOnce(discord_client::Client) -> F,
        refusal: fn(discord_core::Error) -> String,
    ) where
        F: Future<Output = discord_core::Result<SpaceId>> + Send + 'static,
    {
        let Some(linked) = self.current() else {
            return;
        };
        let key = linked.key().to_owned();
        let client = linked.client().clone();
        self.space_sheet.start();
        spawn(
            ask(client),
            move |chat: &mut Chat, added: discord_core::Result<SpaceId>| match added {
                Ok(space) => chat.space_added(&key, space),
                Err(error) => chat.space_sheet.failed(refusal(error)),
            },
        );
    }

    fn space_added(&mut self, key: &str, space: SpaceId) {
        self.space_sheet.wait_for(key, space);
        let known = self
            .linked(key)
            .is_some_and(|linked| linked.space(&space).is_some());
        if known {
            self.space_arrived(key, &space);
        }
    }
}

fn join_refusal(error: discord_core::Error) -> String {
    match error {
        discord_core::Error::Missing(_) => tr!("discord.space-sheet.expired-invite").to_string(),
        error => error.to_string(),
    }
}
