use discord_core::{SpaceId, Template, VANITY_URL, Vanity, Widget};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::overview::{channel_combo, channels};
use super::{Edited, SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::time::sent_at;
use crate::manage::Prompt;

const LONGEST_VANITY: usize = 25;

fn copy_row(label: Label, value: String) -> Element {
    let copied = value.clone();
    column! {
        text(label).caption().semibold().muted();
        row! {
            text(value).label().grow(1.0).min_width(0.0);
            button(text(tr!("discord.manage.copy")))
                .secondary()
                .small()
                .on_press(move || {
                    set_text_of(Selection::Clipboard, &copied);
                    skip_view();
                });
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(super) fn widget(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let id = *space;
    loading(&admin.widget, |edited| {
        let widget = &edited.draft;
        column! {
            field(
                tr!("discord.admin.widget-enable"),
                switch(widget.enabled).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                    admin.edit_widget(|widget| widget.enabled = on);
                }),
            );
            field(
                tr!("discord.admin.widget-channel"),
                channel_combo(
                    widget.channel,
                    channels(linked, space, false),
                    Some(tr!("discord.admin.no-invite-channel")),
                    |admin, chosen| admin.edit_widget(|widget| widget.channel = chosen),
                ),
            );
            copy_row(tr!("discord.admin.space-id"), id.to_string());
            copy_row(
                tr!("discord.admin.widget-json"),
                format!("https://discord.com/api/guilds/{id}/widget.json"),
            );
            copy_row(
                tr!("discord.admin.widget-html"),
                format!(
                    "<iframe src=\"https://discord.com/widget?id={id}&theme=dark\" width=\"350\" height=\"500\" allowtransparency=\"true\" frameborder=\"0\" sandbox=\"allow-popups allow-popups-to-escape-sandbox allow-same-origin allow-scripts\"></iframe>"
                ),
            );
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

pub(super) fn template(admin: &SpaceAdmin) -> Element {
    loading(&admin.templates, |templates| {
        let body = match templates.first() {
            Some(template) => existing(template),
            None => column! {
                text(tr!("discord.admin.template-name")).caption().semibold().muted();
                text_field(&admin.docs.template_name).max_len(100).fill_width();
                text(tr!("discord.admin.template-description")).caption().semibold().muted();
                text_field(&admin.docs.template_description).max_len(120).fill_width();
                button(text(tr!("discord.admin.template-generate")))
                    .primary()
                    .on_press(|chat: &mut Chat| chat.create_template());
            }
            .gap(Space::SNUG)
            .fill_width()
            .any(),
        };
        column! {
            hint(tr!("discord.admin.template-hint"));
            body;
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

fn existing(template: &Template) -> Element {
    let (synced, deleted, opened) = (template.code.clone(), template.code.clone(), template.url());
    column! {
        text(template.name.clone()).heading().semibold();
        if let Some(description) = &template.description {
            text(description.clone()).label().muted().fill_width();
        }
        copy_row(tr!("discord.admin.template-link"), template.url());
        text(tr!("discord.admin.template-uses", count = i64::from(template.uses))).caption().muted();
        if let Some(updated) = template.updated {
            text(tr!("discord.admin.template-updated", time = sent_at(updated).as_str())).caption().muted();
        }
        if template.dirty {
            hint(tr!("discord.admin.template-dirty"));
        }
        row! {
            button(text(tr!("discord.admin.template-sync")))
                .primary()
                .small()
                .enabled(template.dirty)
                .on_press(move |chat: &mut Chat| chat.sync_template(&synced));
            button(text(tr!("discord.admin.template-preview")))
                .secondary()
                .small()
                .on_press(move |chat: &mut Chat| chat.open_page(&opened));
            button(text(tr!("discord.admin.template-delete")))
                .danger()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_delete_template(&deleted));
        }
        .gap(Space::SNUG);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub(super) fn vanity(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let unlocked = linked
        .space(space)
        .is_some_and(|known| known.has(VANITY_URL));
    if !unlocked {
        return column! {
            hint(tr!("discord.admin.vanity-locked"));
            button(text(tr!("discord.admin.boost")))
                .secondary()
                .on_press(|chat: &mut Chat| chat.open_page(super::section::BOOSTING));
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any();
    }
    loading(&admin.vanity, |edited| {
        column! {
            hint(tr!("discord.admin.vanity-hint"));
            row! {
                text("discord.gg/").label().muted();
                text_field(&admin.docs.vanity)
                    .max_len(LONGEST_VANITY)
                    .on_change(|admin: &mut SpaceAdmin| {
                        let code = Some(admin.docs.vanity.text().trim().to_owned()).filter(|code| !code.is_empty());
                        admin.edit_vanity(|vanity| vanity.code = code);
                    })
                    .grow(1.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width();
            text(tr!("discord.admin.vanity-uses", count = i64::from(edited.draft.uses))).caption().muted();
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    })
}

impl SpaceAdmin {
    fn edit_widget(&mut self, change: impl FnOnce(&mut Widget)) {
        if let Some(Ok(mut edited)) = (*self.widget).clone() {
            change(&mut edited.draft);
            self.widget.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }

    fn edit_vanity(&mut self, change: impl FnOnce(&mut Vanity)) {
        if let Some(Ok(mut edited)) = (*self.vanity).clone() {
            change(&mut edited.draft);
            self.vanity.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }
}

impl Chat {
    pub(super) fn load_widget(&mut self) {
        self.fetch(
            |client, space| async move { client.widget(&space).await },
            |admin, widget| admin.widget.set(Some(widget.map(Edited::new))),
        );
    }

    pub(super) fn load_templates(&mut self) {
        self.fetch(
            |client, space| async move { client.templates(&space).await },
            |admin, templates| admin.templates.set(Some(templates)),
        );
    }

    pub(super) fn load_vanity(&mut self) {
        let unlocked = self
            .admin_target()
            .and_then(|(key, space)| {
                self.linked(&key)
                    .and_then(|linked| linked.space(&space))
                    .map(|known| known.has(VANITY_URL))
            })
            .unwrap_or(false);
        if !unlocked {
            return;
        }
        self.fetch(
            |client, space| async move { client.vanity(&space).await },
            |admin, vanity| {
                if let Ok(loaded) = &vanity {
                    admin
                        .docs
                        .vanity
                        .reset(loaded.code.as_deref().unwrap_or_default());
                }
                admin.vanity.set(Some(vanity.map(Edited::new)));
            },
        );
    }

    pub(super) fn save_widget(&mut self) {
        self.save_edited(
            |admin| &mut admin.widget,
            |client, space, widget: Widget| async move { client.save_widget(&space, &widget).await },
        );
    }

    pub(super) fn save_vanity(&mut self) {
        self.save_edited(
            |admin| &mut admin.vanity,
            |client, space, vanity: Vanity| async move {
                client
                    .save_vanity(&space, vanity.code.as_deref().unwrap_or_default())
                    .await
            },
        );
    }

    fn create_template(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let docs = &self.space_admin().docs;
        let (name, description) = (docs.template_name.text(), docs.template_description.text());
        if name.trim().is_empty() {
            return;
        }
        self.act(
            &key,
            move |client| {
                let (name, description) = (name.clone(), description.clone());
                async move { client.create_template(&space, &name, &description).await }
            },
            |chat: &mut Chat, _: Template| chat.load_templates(),
        );
    }

    fn sync_template(&mut self, code: &str) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let code = code.to_owned();
        self.act(
            &key,
            move |client| {
                let code = code.clone();
                async move { client.sync_template(&space, &code).await }
            },
            |chat: &mut Chat, _: Template| chat.load_templates(),
        );
    }

    fn ask_delete_template(&mut self, code: &str) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let code = code.to_owned();
        self.confirm(
            Prompt {
                title: tr!("discord.admin.template-delete").to_string(),
                body: tr!("discord.admin.template-delete-body").to_string(),
                accept: tr!("discord.manage.delete").to_string(),
            },
            move |chat: &mut Chat| {
                let code = code.clone();
                chat.act(
                    &key,
                    move |client| {
                        let code = code.clone();
                        async move { client.delete_template(&space, &code).await }
                    },
                    |chat: &mut Chat, ()| chat.load_templates(),
                );
            },
        );
    }
}
