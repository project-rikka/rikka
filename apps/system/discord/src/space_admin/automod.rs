use discord_core::{
    AutomodRule, ChannelId, KeywordPreset, MAX_KEYWORD_RULES, RoleId, RuleAction, RuleTrigger,
    SpaceId,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::docs::{joined, listed};
use super::overview::{channel_combo, channels};
use super::{SpaceAdmin, hint, loading, picks, toggle};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::{Prompt, timeout_label};

const SINGLETONS: [RuleTrigger; 4] = [
    RuleTrigger::MentionSpam,
    RuleTrigger::Spam,
    RuleTrigger::KeywordPreset,
    RuleTrigger::MemberProfile,
];
const TIMEOUTS: [u32; 6] = [60, 300, 600, 3600, 86_400, 604_800];

fn trigger_title(trigger: RuleTrigger) -> Label {
    match trigger {
        RuleTrigger::Keyword => tr!("discord.automod.keyword"),
        RuleTrigger::Spam => tr!("discord.automod.spam"),
        RuleTrigger::KeywordPreset => tr!("discord.automod.preset"),
        RuleTrigger::MentionSpam => tr!("discord.automod.mention-spam"),
        RuleTrigger::MemberProfile => tr!("discord.automod.profile"),
    }
}

fn trigger_hint(trigger: RuleTrigger) -> Label {
    match trigger {
        RuleTrigger::Keyword => tr!("discord.automod.keyword-hint"),
        RuleTrigger::Spam => tr!("discord.automod.spam-hint"),
        RuleTrigger::KeywordPreset => tr!("discord.automod.preset-hint"),
        RuleTrigger::MentionSpam => tr!("discord.automod.mention-spam-hint"),
        RuleTrigger::MemberProfile => tr!("discord.automod.profile-hint"),
    }
}

fn preset_label(preset: KeywordPreset) -> Label {
    match preset {
        KeywordPreset::Profanity => tr!("discord.automod.profanity"),
        KeywordPreset::SexualContent => tr!("discord.automod.sexual"),
        KeywordPreset::Slurs => tr!("discord.automod.slurs"),
    }
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    if let Some(rule) = admin.rule.as_ref() {
        return editor(admin, linked, space, rule);
    }
    loading(&admin.rules, |rules| {
        let custom: Vec<&AutomodRule> = rules
            .iter()
            .filter(|rule| rule.trigger == RuleTrigger::Keyword)
            .collect();
        column! {
            hint(tr!("discord.automod.hint"));
            column_of(SINGLETONS.map(|trigger| {
                let rule = rules.iter().find(|rule| rule.trigger == trigger);
                rule_card(trigger, rule)
            }))
            .gap(Space::SNUG)
            .fill_width();
            section(
                tr!("discord.automod.custom", count = custom.len() as i64, limit = MAX_KEYWORD_RULES as i64),
                column! {
                    column_of(custom.iter().map(|rule| rule_card(RuleTrigger::Keyword, Some(rule))))
                        .gap(Space::SNUG)
                        .fill_width();
                    button(text(tr!("discord.automod.create-custom")))
                        .secondary()
                        .small()
                        .enabled(custom.len() < MAX_KEYWORD_RULES)
                        .on_press(|chat: &mut Chat| chat.draft_rule(RuleTrigger::Keyword, None));
                }
                .gap(Space::SNUG)
                .fill_width(),
            );
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn rule_card(trigger: RuleTrigger, rule: Option<&AutomodRule>) -> Element {
    let title = rule.map_or_else(
        || trigger_title(trigger),
        |rule| Label::from(rule.name.clone()),
    );
    let edited = rule.cloned();
    let toggled = rule.cloned();
    card(
        row! {
            column! {
                text(title).label().semibold();
                text(trigger_hint(trigger)).caption().muted().fill_width();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            if let Some(rule) = rule {
                switch(rule.enabled).on_toggle(move |chat: &mut Chat, on: bool| {
                    if let Some(mut rule) = toggled.clone() {
                        rule.enabled = on;
                        chat.save_rule(rule);
                    }
                });
            }
            button(text(match rule {
                Some(_) => tr!("discord.admin.edit"),
                None => tr!("discord.automod.set-up"),
            }))
            .secondary()
            .small()
            .on_press(move |chat: &mut Chat| chat.draft_rule(trigger, edited.clone()));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

fn action_of(rule: &AutomodRule, matches: fn(&RuleAction) -> bool) -> Option<&RuleAction> {
    rule.actions.iter().find(|action| matches(action))
}

fn editor(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    rule: &AutomodRule,
) -> Element {
    let trigger = rule.trigger;
    let lists = matches!(trigger, RuleTrigger::Keyword | RuleTrigger::MemberProfile);
    let allows = matches!(
        trigger,
        RuleTrigger::Keyword | RuleTrigger::KeywordPreset | RuleTrigger::MemberProfile
    );
    let profile = trigger == RuleTrigger::MemberProfile;
    let timeouts = matches!(trigger, RuleTrigger::Keyword | RuleTrigger::MentionSpam);
    let block = action_of(rule, |action| matches!(action, RuleAction::Block { .. })).is_some();
    let alert =
        action_of(rule, |action| matches!(action, RuleAction::Alert { .. })).and_then(|action| {
            match action {
                RuleAction::Alert { channel } => Some(*channel),
                _ => None,
            }
        });
    let timeout =
        action_of(rule, |action| matches!(action, RuleAction::Timeout { .. })).and_then(|action| {
            match action {
                RuleAction::Timeout { seconds } => Some(*seconds),
                _ => None,
            }
        });
    let deletable = rule.id.is_some() && trigger == RuleTrigger::Keyword;
    let roles: Vec<(RoleId, String)> = {
        let mut roles: Vec<&discord_core::Role> = linked
            .roles(space)
            .iter()
            .filter(|role| !role.is_everyone(space))
            .collect();
        roles.sort_by_key(|role| std::cmp::Reverse(role.position));
        roles
            .into_iter()
            .map(|role| (role.id, role.name.clone()))
            .collect()
    };
    let text_channels = channels(linked, space, false);
    column! {
        text(trigger_title(trigger)).heading().semibold();
        text(tr!("discord.automod.rule-name")).caption().semibold().muted();
        text_field(&admin.docs.rule_name).max_len(100).fill_width();
        if lists {
            text(tr!("discord.automod.keywords")).caption().semibold().muted();
            framed(text_editor(&admin.docs.rule_keywords).wrap(true).fits_text(60.0..=160.0));
            hint(tr!("discord.automod.keywords-hint"));
            text(tr!("discord.automod.patterns")).caption().semibold().muted();
            framed(text_editor(&admin.docs.rule_patterns).wrap(true).fits_text(40.0..=120.0));
        }
        if trigger == RuleTrigger::KeywordPreset {
            column_of(KeywordPreset::ALL.map(|preset| {
                checkbox(rule.data.presets.contains(&preset))
                    .label(text(preset_label(preset)).label())
                    .on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                        admin.edit_rule(|rule| {
                            rule.data.presets.retain(|known| *known != preset);
                            if on {
                                rule.data.presets.push(preset);
                            }
                        });
                    })
                    .any()
            }))
            .gap(Space::TIGHT);
        }
        if allows {
            text(tr!("discord.automod.allowed")).caption().semibold().muted();
            framed(text_editor(&admin.docs.rule_allowed).wrap(true).fits_text(40.0..=120.0));
        }
        if trigger == RuleTrigger::MentionSpam {
            field(
                tr!("discord.automod.mention-limit"),
                number_field(rule.data.mention_limit.unwrap_or(discord_core::MENTION_LIMIT) as f32)
                    .range(1.0..=50.0)
                    .step(1.0)
                    .decimals(0)
                    .on_commit(|admin: &mut SpaceAdmin, limit: f32| {
                        admin.edit_rule(|rule| rule.data.mention_limit = Some(limit.round() as u32));
                    })
                    .width(120.0),
            );
            field(
                tr!("discord.automod.raid-protection"),
                switch(rule.data.raid_protection).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                    admin.edit_rule(|rule| rule.data.raid_protection = on);
                }),
            );
        }
        section(
            tr!("discord.automod.actions"),
            column! {
                if profile {
                    hint(tr!("discord.automod.profile-action"));
                } else {
                    field(
                        tr!("discord.automod.block"),
                        switch(block).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                            admin.set_action(|action| matches!(action, RuleAction::Block { .. }), on.then_some(RuleAction::Block { message: None }));
                        }),
                    );
                    if block {
                        text(tr!("discord.automod.block-message")).caption().semibold().muted();
                        text_field(&admin.docs.rule_message).max_len(150).fill_width();
                    }
                    field(
                        tr!("discord.automod.alert"),
                        switch(alert.is_some()).enabled(!text_channels.is_empty()).on_toggle({
                            let first = text_channels.first().map(|(id, _)| *id);
                            move |admin: &mut SpaceAdmin, on: bool| {
                                let action = first.filter(|_| on).map(|channel| RuleAction::Alert { channel });
                                admin.set_action(|action| matches!(action, RuleAction::Alert { .. }), action);
                            }
                        }),
                    );
                    if let Some(channel) = alert {
                        channel_combo(Some(channel), text_channels.clone(), None, |admin, chosen| {
                            if let Some(channel) = chosen {
                                admin.set_action(|action| matches!(action, RuleAction::Alert { .. }), Some(RuleAction::Alert { channel }));
                            }
                        });
                    }
                    if timeouts {
                        field(
                            tr!("discord.automod.timeout"),
                            switch(timeout.is_some()).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                                let action = on.then_some(RuleAction::Timeout { seconds: TIMEOUTS[0] });
                                admin.set_action(|action| matches!(action, RuleAction::Timeout { .. }), action);
                            }),
                        );
                        if let Some(seconds) = timeout {
                            combo(TIMEOUTS.iter().position(|known| *known == seconds))
                                .options(discord_core::TimeoutSpan::ALL.map(timeout_label))
                                .on_select(|admin: &mut SpaceAdmin, at: usize| {
                                    let seconds = TIMEOUTS[at.min(TIMEOUTS.len() - 1)];
                                    admin.set_action(|action| matches!(action, RuleAction::Timeout { .. }), Some(RuleAction::Timeout { seconds }));
                                })
                                .width(200.0);
                        }
                    }
                }
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
        section(
            tr!("discord.automod.exempt-roles"),
            picks(roles, &rule.exempt_roles, |admin, role, on| {
                admin.edit_rule(|rule| toggle(&mut rule.exempt_roles, role, on));
            }),
        );
        if !profile {
            section(
                tr!("discord.automod.exempt-channels"),
                picks(text_channels, &rule.exempt_channels, |admin, channel: ChannelId, on| {
                    admin.edit_rule(|rule| toggle(&mut rule.exempt_channels, channel, on));
                }),
            );
        }
        row! {
            if deletable {
                button(text(tr!("discord.automod.delete")))
                    .danger()
                    .on_press(|chat: &mut Chat| chat.ask_delete_rule());
            }
            spacer().grow(1.0);
            button(text(tr!("discord.manage.cancel")))
                .ghost()
                .on_press(|admin: &mut SpaceAdmin| admin.rule.set(None));
            button(text(tr!("discord.manage.save")))
                .primary()
                .enabled(!admin.is_saving())
                .on_press(|chat: &mut Chat| chat.submit_rule());
        }
        .gap(Space::SNUG)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn framed(editor: impl View) -> Element {
    container(editor)
        .padding(Inset::all(Space::SNUG))
        .border(1.0, palette().outline)
        .radius(Corner::Control)
        .fill_width()
        .any()
}

impl SpaceAdmin {
    fn edit_rule(&mut self, change: impl FnOnce(&mut AutomodRule)) {
        if let Some(mut rule) = (*self.rule).clone() {
            change(&mut rule);
            self.rule.set(Some(rule));
        }
    }

    fn set_action(&mut self, kind: fn(&RuleAction) -> bool, action: Option<RuleAction>) {
        self.edit_rule(|rule| {
            let at = rule.actions.iter().position(kind);
            match (at, action) {
                (Some(at), Some(action)) => rule.actions[at] = action,
                (Some(at), None) => {
                    rule.actions.remove(at);
                }
                (None, Some(action)) => rule.actions.push(action),
                (None, None) => {}
            }
        });
    }

    fn open_rule(&mut self, rule: AutomodRule) {
        self.docs.rule_name.reset(&rule.name);
        self.docs.rule_keywords.reset(joined(&rule.data.keywords));
        self.docs.rule_patterns.reset(rule.data.patterns.join("\n"));
        self.docs.rule_allowed.reset(joined(&rule.data.allowed));
        let message = rule.actions.iter().find_map(|action| match action {
            RuleAction::Block { message } => message.clone(),
            _ => None,
        });
        self.docs
            .rule_message
            .reset(message.as_deref().unwrap_or_default());
        self.rule.set(Some(rule));
    }

    fn collected_rule(&self) -> Option<AutomodRule> {
        let mut rule = self.rule.as_ref()?.clone();
        rule.name = self.docs.rule_name.text().trim().to_owned();
        let patterns: Vec<String> = self
            .docs
            .rule_patterns
            .text()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        rule.data.keywords = listed(&self.docs.rule_keywords);
        rule.data.patterns = patterns;
        rule.data.allowed = listed(&self.docs.rule_allowed);
        let message =
            Some(self.docs.rule_message.text().trim().to_owned()).filter(|text| !text.is_empty());
        for action in &mut rule.actions {
            if let RuleAction::Block { message: shown } = action {
                *shown = message.clone();
            }
        }
        if rule.trigger == RuleTrigger::MemberProfile {
            rule.actions = vec![RuleAction::BlockInteractions];
        }
        Some(rule)
    }
}

impl Chat {
    pub(super) fn load_rules(&mut self) {
        self.fetch(
            |client, space| async move { client.automod_rules(&space).await },
            |admin, rules| admin.rules.set(Some(rules)),
        );
    }

    fn draft_rule(&mut self, trigger: RuleTrigger, existing: Option<AutomodRule>) {
        let rule = existing
            .unwrap_or_else(|| AutomodRule::draft(trigger, trigger_title(trigger).to_string()));
        self.space_admin_mut().open_rule(rule);
    }

    fn submit_rule(&mut self) {
        if let Some(rule) = self.space_admin().collected_rule() {
            self.save_rule(rule);
        }
    }

    fn save_rule(&mut self, rule: AutomodRule) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        self.space_admin_mut().saving.set(true);
        self.attempt(
            &key,
            move |client| {
                let rule = rule.clone();
                async move { client.save_automod_rule(&space, &rule).await }
            },
            move |chat: &mut Chat, saved: Option<AutomodRule>| {
                chat.admin_saved(space);
                if saved.is_some() {
                    chat.space_admin_mut().rule.set(None);
                    chat.load_rules();
                }
            },
        );
    }

    fn ask_delete_rule(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(rule) = self.space_admin().rule.as_ref().and_then(|rule| rule.id) else {
            return;
        };
        self.confirm(
            Prompt {
                title: tr!("discord.automod.delete").to_string(),
                body: tr!("discord.automod.delete-body").to_string(),
                accept: tr!("discord.manage.delete").to_string(),
            },
            move |chat: &mut Chat| {
                chat.act(
                    &key,
                    move |client| async move { client.delete_automod_rule(&space, &rule).await },
                    |chat: &mut Chat, ()| {
                        chat.space_admin_mut().rule.set(None);
                        chat.load_rules();
                    },
                );
            },
        );
    }
}
