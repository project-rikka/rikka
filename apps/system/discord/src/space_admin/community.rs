use discord_core::{
    COMMUNITY, ContentFilter, LOCALES, MAX_WELCOME_CHANNELS, SpaceEdit, SpaceId, SpaceSettings,
    VerificationLevel, WELCOME_SCREEN_ENABLED, WelcomeChannel, WelcomeScreen,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::overview::{channel_combo, channels};
use super::{Edited, SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::Prompt;

const LONGEST_DESCRIPTION: usize = 120;
const LONGEST_WELCOME: usize = 140;
const CHOICE: f32 = 260.0;

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    loading(&admin.settings, |edited| {
        let settings = &edited.draft;
        match edited.original.is_community() {
            true => enabled(admin, linked, space, settings),
            false => enabling(admin, linked, space),
        }
    })
}

fn enabling(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let text_channels = channels(linked, space, false);
    let ready = admin.community_rules.is_some() && admin.community_updates.is_some();
    column! {
        text(tr!("discord.community.title")).heading().semibold();
        hint(tr!("discord.community.hint"));
        section(
            tr!("discord.community.requirements"),
            column! {
                hint(tr!("discord.community.verification"));
                hint(tr!("discord.community.filter"));
            }
            .gap(Space::TIGHT)
            .fill_width(),
        );
        field(
            tr!("discord.community.rules-channel"),
            channel_combo(*admin.community_rules, text_channels.clone(), None, |admin, chosen| {
                admin.community_rules.set(chosen);
            }),
        );
        field(
            tr!("discord.community.updates-channel"),
            channel_combo(*admin.community_updates, text_channels, None, |admin, chosen| {
                admin.community_updates.set(chosen);
            }),
        );
        button(text(tr!("discord.community.enable")))
            .primary()
            .enabled(ready)
            .on_press(|chat: &mut Chat| chat.enable_community());
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn enabled(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    settings: &SpaceSettings,
) -> Element {
    let text_channels = channels(linked, space, false);
    let locale_at = LOCALES.iter().position(|locale| *locale == settings.locale);
    column! {
        field(
            tr!("discord.community.rules-channel"),
            channel_combo(settings.rules_channel, text_channels.clone(), None, |admin, chosen| {
                admin.edit_settings(|settings| settings.rules_channel = chosen);
            }),
        );
        field(
            tr!("discord.community.updates-channel"),
            channel_combo(settings.updates_channel, text_channels, None, |admin, chosen| {
                admin.edit_settings(|settings| settings.updates_channel = chosen);
            }),
        );
        field(
            tr!("discord.community.locale"),
            combo(locale_at)
                .options(LOCALES.map(Label::from))
                .on_select(|admin: &mut SpaceAdmin, at: usize| {
                    let locale = LOCALES[at.min(LOCALES.len() - 1)].to_owned();
                    admin.edit_settings(|settings| settings.locale = locale);
                })
                .width(CHOICE),
        );
        text(tr!("discord.community.description")).caption().semibold().muted();
        text_field(&admin.docs.description)
            .max_len(LONGEST_DESCRIPTION)
            .on_change(|admin: &mut SpaceAdmin| {
                let description = Some(admin.docs.description.text().trim().to_owned()).filter(|text| !text.is_empty());
                admin.edit_settings(|settings| settings.description = description);
            })
            .fill_width();
        hint(tr!("discord.community.description-hint"));
        section(
            tr!("discord.community.disable-title"),
            column! {
                hint(tr!("discord.community.disable-hint"));
                button(text(tr!("discord.community.disable")))
                    .danger()
                    .on_press(|chat: &mut Chat| chat.ask_disable_community());
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

pub(super) fn welcome(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let text_channels = channels(linked, space, false);
    loading(&admin.welcome, |edited| {
        let screen = &edited.draft;
        let rows = screen.channels.iter().enumerate().map(|(at, entry)| {
            let docs = &admin.docs.welcome_channels[at];
            row! {
                channel_combo(Some(entry.channel), text_channels.clone(), None, move |admin, chosen| {
                    if let Some(channel) = chosen {
                        admin.edit_welcome(|screen| screen.channels[at].channel = channel);
                    }
                });
                text_field(docs)
                    .placeholder(tr!("discord.community.welcome-channel-hint"))
                    .max_len(50)
                    .on_change(move |admin: &mut SpaceAdmin| {
                        let description = admin.docs.welcome_channels[at].text();
                        admin.edit_welcome(|screen| screen.channels[at].description = description);
                    })
                    .grow(1.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .on_press(move |admin: &mut SpaceAdmin| admin.remove_welcome(at));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .any()
        });
        let first = text_channels.first().map(|(id, _)| *id);
        column! {
            hint(tr!("discord.community.welcome-hint"));
            field(
                tr!("discord.community.welcome-enable"),
                switch(screen.enabled).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                    admin.edit_welcome(|screen| screen.enabled = on);
                }),
            );
            text(tr!("discord.community.welcome-description")).caption().semibold().muted();
            text_field(&admin.docs.welcome_description)
                .max_len(LONGEST_WELCOME)
                .on_change(|admin: &mut SpaceAdmin| {
                    let description = admin.docs.welcome_description.text();
                    admin.edit_welcome(|screen| screen.description = description);
                })
                .fill_width();
            text(tr!("discord.community.welcome-channels")).caption().semibold().muted();
            column_of(rows).gap(Space::SNUG).fill_width();
            button(text(tr!("discord.community.welcome-add")))
                .secondary()
                .small()
                .enabled(screen.channels.len() < MAX_WELCOME_CHANNELS && first.is_some())
                .on_press(move |admin: &mut SpaceAdmin| {
                    if let Some(channel) = first {
                        admin.add_welcome(channel);
                    }
                });
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    })
}

impl SpaceAdmin {
    fn edit_welcome(&mut self, change: impl FnOnce(&mut WelcomeScreen)) {
        if let Some(Ok(mut edited)) = (*self.welcome).clone() {
            change(&mut edited.draft);
            self.welcome.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }

    fn add_welcome(&mut self, channel: discord_core::ChannelId) {
        self.edit_welcome(|screen| {
            screen.channels.push(WelcomeChannel {
                channel,
                description: String::new(),
                emoji: None,
            });
        });
        self.sync_welcome_docs();
    }

    fn remove_welcome(&mut self, at: usize) {
        self.edit_welcome(|screen| {
            if at < screen.channels.len() {
                screen.channels.remove(at);
            }
        });
        self.sync_welcome_docs();
    }

    pub(super) fn sync_welcome_docs(&mut self) {
        let Some(Ok(edited)) = self.welcome.as_ref() else {
            return;
        };
        let screen = edited.draft.clone();
        self.docs.welcome_description.reset(&screen.description);
        for (at, docs) in self.docs.welcome_channels.iter_mut().enumerate() {
            docs.reset(
                screen
                    .channels
                    .get(at)
                    .map_or("", |entry| entry.description.as_str()),
            );
        }
    }
}

impl Chat {
    pub(super) fn load_welcome(&mut self) {
        let enabled = self
            .admin_target()
            .and_then(|(key, space)| {
                self.linked(&key)?
                    .space(&space)
                    .map(|known| known.has(WELCOME_SCREEN_ENABLED))
            })
            .unwrap_or(false);
        self.fetch(
            move |client, space| async move { client.welcome_screen(&space, enabled).await },
            |admin, screen| {
                admin.welcome.set(Some(screen.map(Edited::new)));
                admin.sync_welcome_docs();
            },
        );
    }

    pub(super) fn save_welcome(&mut self) {
        self.save_edited(
            |admin| &mut admin.welcome,
            |client, space, screen: WelcomeScreen| async move {
                client.save_welcome_screen(&space, &screen).await
            },
        );
    }

    fn enable_community(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let admin = self.space_admin();
        let (Some(rules), Some(updates)) = (*admin.community_rules, *admin.community_updates)
        else {
            return;
        };
        let Some(settings) = admin.settings_original().cloned() else {
            return;
        };
        let edits = vec![
            SpaceEdit::Features(settings.with_feature(COMMUNITY, true)),
            SpaceEdit::Verification(at_least_low(settings.verification)),
            SpaceEdit::ContentFilter(ContentFilter::AllMembers),
            SpaceEdit::RulesChannel(Some(rules)),
            SpaceEdit::UpdatesChannel(Some(updates)),
        ];
        self.community_edit(&key, space, edits);
    }

    fn ask_disable_community(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(settings) = self.space_admin().settings_original().cloned() else {
            return;
        };
        self.confirm(
            Prompt {
                title: tr!("discord.community.disable-title").to_string(),
                body: tr!("discord.community.disable-hint").to_string(),
                accept: tr!("discord.community.disable").to_string(),
            },
            move |chat: &mut Chat| {
                let edits = vec![SpaceEdit::Features(settings.with_feature(COMMUNITY, false))];
                chat.community_edit(&key, space, edits);
            },
        );
    }

    fn community_edit(&mut self, key: &str, space: SpaceId, edits: Vec<SpaceEdit>) {
        self.act(
            key,
            move |client| {
                let edits = edits.clone();
                async move { client.edit_space(&space, edits).await }
            },
            move |chat: &mut Chat, settings: SpaceSettings| {
                if let Some(admin) = chat.admin_for(space) {
                    admin.settings_changed(settings, |draft, saved| {
                        draft.features = saved.features.clone();
                        draft.verification = saved.verification;
                        draft.content_filter = saved.content_filter;
                        draft.rules_channel = saved.rules_channel;
                        draft.updates_channel = saved.updates_channel;
                    });
                }
            },
        );
    }
}

fn at_least_low(level: VerificationLevel) -> VerificationLevel {
    match level {
        VerificationLevel::None => VerificationLevel::Low,
        level => level,
    }
}
