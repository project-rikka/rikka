use discord_core::{
    ChannelId, MIN_DEFAULT_CHANNELS, MessageId, Onboarding, OnboardingMode, Prompt, PromptOption,
    RoleId, SpaceId,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::overview::channels;
use super::{Edited, SpaceAdmin, hint, loading, picks, toggle};
use crate::account::LinkedAccount;
use crate::chat::{Chat, now_ms};

const LONGEST_TITLE: usize = 100;
const LONGEST_OPTION: usize = 50;
const LONGEST_DESCRIPTION: usize = 100;

fn minted(offset: u64) -> String {
    (MessageId::minted_at(now_ms()).get() + offset).to_string()
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let text_channels = channels(linked, space, false);
    let mut roles: Vec<&discord_core::Role> = linked
        .roles(space)
        .iter()
        .filter(|role| !role.is_everyone(space) && !role.managed)
        .collect();
    roles.sort_by_key(|role| std::cmp::Reverse(role.position));
    let roles: Vec<(RoleId, String)> = roles
        .into_iter()
        .map(|role| (role.id, role.name.clone()))
        .collect();
    loading(&admin.onboarding, |edited| {
        let onboarding = &edited.draft;
        if let Some(at) = *admin.prompt_at
            && let Some(prompt) = onboarding.prompts.get(at)
        {
            return prompt_editor(admin, at, prompt, &text_channels, &roles);
        }
        let mode_at = usize::from(onboarding.mode == OnboardingMode::Advanced);
        column! {
            hint(tr!("discord.onboarding.hint"));
            field(
                tr!("discord.onboarding.enable"),
                switch(onboarding.enabled).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                    admin.edit_onboarding(|onboarding| onboarding.enabled = on);
                }),
            );
            field(
                tr!("discord.onboarding.mode"),
                segmented(mode_at)
                    .segment(tr!("discord.onboarding.mode-default"))
                    .segment(tr!("discord.onboarding.mode-advanced"))
                    .on_select(|admin: &mut SpaceAdmin, at: usize| {
                        let mode = match at {
                            0 => OnboardingMode::Default,
                            _ => OnboardingMode::Advanced,
                        };
                        admin.edit_onboarding(|onboarding| onboarding.mode = mode);
                    }),
            );
            section(
                tr!("discord.onboarding.default-channels", count = onboarding.default_channels.len() as i64, least = MIN_DEFAULT_CHANNELS as i64),
                picks(text_channels.clone(), &onboarding.default_channels, |admin, channel, on| {
                    admin.edit_onboarding(|onboarding| toggle(&mut onboarding.default_channels, channel, on));
                }),
            );
            section(
                tr!("discord.onboarding.questions"),
                column! {
                    column_of(onboarding.prompts.iter().enumerate().map(|(at, prompt)| prompt_card(at, prompt)))
                        .gap(Space::SNUG)
                        .fill_width();
                    button(text(tr!("discord.onboarding.add-question")))
                        .secondary()
                        .small()
                        .on_press(|admin: &mut SpaceAdmin| admin.add_prompt());
                }
                .gap(Space::SNUG)
                .fill_width(),
            );
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

fn prompt_card(at: usize, prompt: &Prompt) -> Element {
    card(
        row! {
            column! {
                text(match prompt.title.is_empty() {
                    true => tr!("discord.onboarding.untitled"),
                    false => Label::from(prompt.title.clone()),
                })
                .label()
                .semibold();
                text(tr!("discord.onboarding.option-count", count = prompt.options.len() as i64)).caption().muted();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(text(tr!("discord.admin.edit")))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| admin.open_prompt(at));
            button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| {
                    admin.edit_onboarding(|onboarding| {
                        if at < onboarding.prompts.len() {
                            onboarding.prompts.remove(at);
                        }
                    });
                });
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

fn prompt_editor(
    admin: &SpaceAdmin,
    at: usize,
    prompt: &Prompt,
    text_channels: &[(ChannelId, String)],
    roles: &[(RoleId, String)],
) -> Element {
    let options = prompt.options.iter().enumerate().map(|(index, option)| {
        let (Some(title), Some(description)) = (
            admin.docs.option_titles.get(index),
            admin.docs.option_descriptions.get(index),
        ) else {
            return spacer().any();
        };
        card(
            column! {
                row! {
                    text_field(title)
                        .placeholder(tr!("discord.onboarding.option-title"))
                        .max_len(LONGEST_OPTION)
                        .on_change(move |admin: &mut SpaceAdmin| {
                            let text = admin.docs.option_titles[index].text();
                            admin.edit_prompt(at, |prompt| prompt.options[index].title = text);
                        })
                        .grow(1.0);
                    button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                        .ghost()
                        .small()
                        .on_press(move |admin: &mut SpaceAdmin| admin.remove_option(at, index));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
                text_field(description)
                    .placeholder(tr!("discord.onboarding.option-description"))
                    .max_len(LONGEST_DESCRIPTION)
                    .on_change(move |admin: &mut SpaceAdmin| {
                        let text = admin.docs.option_descriptions[index].text();
                        admin.edit_prompt(at, |prompt| prompt.options[index].description = text);
                    })
                    .fill_width();
                text(tr!("discord.onboarding.option-channels")).caption().semibold().muted();
                picks(text_channels.to_vec(), &option.channels, move |admin, channel, on| {
                    admin.edit_prompt(at, |prompt| toggle(&mut prompt.options[index].channels, channel, on));
                });
                text(tr!("discord.onboarding.option-roles")).caption().semibold().muted();
                picks(roles.to_vec(), &option.roles, move |admin, role, on| {
                    admin.edit_prompt(at, |prompt| toggle(&mut prompt.options[index].roles, role, on));
                });
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .any()
    });
    column! {
        row! {
            button(text(tr!("discord.onboarding.back")))
                .ghost()
                .small()
                .on_press(|admin: &mut SpaceAdmin| admin.prompt_at.set(None));
        };
        text(tr!("discord.onboarding.question")).caption().semibold().muted();
        text_field(&admin.docs.prompt_title)
            .max_len(LONGEST_TITLE)
            .on_change(move |admin: &mut SpaceAdmin| {
                let title = admin.docs.prompt_title.text();
                admin.edit_prompt(at, |prompt| prompt.title = title);
            })
            .fill_width();
        field(
            tr!("discord.onboarding.single-select"),
            switch(prompt.single_select).on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                admin.edit_prompt(at, |prompt| prompt.single_select = on);
            }),
        );
        field(
            tr!("discord.onboarding.required"),
            switch(prompt.required).on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                admin.edit_prompt(at, |prompt| prompt.required = on);
            }),
        );
        field(
            tr!("discord.onboarding.in-onboarding"),
            switch(prompt.in_onboarding).on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                admin.edit_prompt(at, |prompt| prompt.in_onboarding = on);
            }),
        );
        field(
            tr!("discord.onboarding.dropdown"),
            switch(prompt.dropdown).on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                admin.edit_prompt(at, |prompt| prompt.dropdown = on);
            }),
        );
        column_of(options).gap(Space::SNUG).fill_width();
        button(text(tr!("discord.onboarding.add-option")))
            .secondary()
            .small()
            .on_press(move |admin: &mut SpaceAdmin| admin.add_option(at));
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

impl SpaceAdmin {
    fn edit_onboarding(&mut self, change: impl FnOnce(&mut Onboarding)) {
        if let Some(Ok(mut edited)) = (*self.onboarding).clone() {
            change(&mut edited.draft);
            self.onboarding.set(Some(Ok(edited)));
            self.warned.set(false);
        }
    }

    fn edit_prompt(&mut self, at: usize, change: impl FnOnce(&mut Prompt)) {
        self.edit_onboarding(|onboarding| {
            if let Some(prompt) = onboarding.prompts.get_mut(at) {
                change(prompt);
            }
        });
    }

    fn open_prompt(&mut self, at: usize) {
        let Some(Ok(edited)) = self.onboarding.as_ref() else {
            return;
        };
        let Some(prompt) = edited.draft.prompts.get(at).cloned() else {
            return;
        };
        self.docs.prompt_title.reset(&prompt.title);
        self.docs.option_titles = prompt
            .options
            .iter()
            .map(|option| TextDocument::new(&option.title))
            .collect();
        self.docs.option_descriptions = prompt
            .options
            .iter()
            .map(|option| TextDocument::new(&option.description))
            .collect();
        self.prompt_at.set(Some(at));
    }

    fn add_prompt(&mut self) {
        let mut added = None;
        self.edit_onboarding(|onboarding| {
            onboarding.prompts.push(Prompt {
                id: minted(0),
                title: String::new(),
                options: Vec::new(),
                single_select: false,
                required: false,
                in_onboarding: true,
                dropdown: false,
            });
            added = Some(onboarding.prompts.len() - 1);
        });
        if let Some(at) = added {
            self.open_prompt(at);
        }
    }

    fn add_option(&mut self, at: usize) {
        let offset = self.docs.option_titles.len() as u64 + 1;
        self.edit_prompt(at, |prompt| {
            prompt.options.push(PromptOption {
                id: minted(offset),
                title: String::new(),
                description: String::new(),
                emoji: None,
                channels: Vec::new(),
                roles: Vec::new(),
            });
        });
        self.docs.option_titles.push(TextDocument::new(""));
        self.docs.option_descriptions.push(TextDocument::new(""));
    }

    fn remove_option(&mut self, at: usize, index: usize) {
        self.edit_prompt(at, |prompt| {
            if index < prompt.options.len() {
                prompt.options.remove(index);
            }
        });
        if index < self.docs.option_titles.len() {
            self.docs.option_titles.remove(index);
            self.docs.option_descriptions.remove(index);
        }
    }
}

impl Chat {
    pub(super) fn load_onboarding(&mut self) {
        self.fetch(
            |client, space| async move { client.onboarding(&space).await },
            |admin, onboarding| admin.onboarding.set(Some(onboarding.map(Edited::new))),
        );
    }

    pub(super) fn save_onboarding(&mut self) {
        self.save_edited(
            |admin| &mut admin.onboarding,
            |client, space, onboarding: Onboarding| async move {
                client
                    .save_onboarding(&space, &onboarding)
                    .await
                    .map(|_| ())
            },
        );
    }
}
