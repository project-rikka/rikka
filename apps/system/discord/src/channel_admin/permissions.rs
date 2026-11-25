use discord_core::{
    ChannelSettings, Grant, Overwrite, OverwriteTarget, Permissions, RoleId, SpaceId,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::ChannelAdmin;
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::permission_catalog::{self, Group, Scope};
use crate::space_admin::{hint, loading};

const LIST: f32 = 220.0;
const GRANTS: [Grant; 3] = [Grant::Deny, Grant::Inherit, Grant::Allow];

fn target_name(linked: &LinkedAccount, space: &SpaceId, target: OverwriteTarget) -> String {
    match target {
        OverwriteTarget::Role(role) => linked
            .role(space, &role)
            .map_or_else(|| role.to_string(), |known| known.name.clone()),
        OverwriteTarget::Member(user) => linked
            .name_of(Some(space), &user)
            .unwrap_or_else(|| user.to_string()),
    }
}

fn everyone(space: &SpaceId) -> OverwriteTarget {
    OverwriteTarget::Role(RoleId::new(space.get()))
}

pub(super) fn view(admin: &ChannelAdmin, linked: &LinkedAccount) -> Element {
    loading(&admin.settings, |edited| {
        let settings = &edited.draft;
        let space = settings.space;
        let mut targets: Vec<OverwriteTarget> = vec![everyone(&space)];
        targets.extend(
            settings
                .overwrites
                .iter()
                .map(|overwrite| overwrite.target)
                .filter(|target| !target.is_everyone(&space)),
        );
        let chosen = admin.target.unwrap_or_else(|| everyone(&space));
        let synced = settings
            .parent
            .and_then(|parent| linked.channel(&parent))
            .is_some();
        let missing: Vec<(RoleId, String)> = {
            let mut roles: Vec<&discord_core::Role> = linked
                .roles(&space)
                .iter()
                .filter(|role| !role.is_everyone(&space))
                .filter(|role| !targets.contains(&OverwriteTarget::Role(role.id)))
                .collect();
            roles.sort_by_key(|role| std::cmp::Reverse(role.position));
            roles
                .into_iter()
                .map(|role| (role.id, role.name.clone()))
                .collect()
        };
        let list = column! {
            if !missing.is_empty() {
                combo(None::<usize>)
                    .placeholder(tr!("discord.channel-admin.add-role"))
                    .options(missing.iter().map(|(_, name)| Label::from(name.clone())).collect::<Vec<_>>())
                    .on_select(move |admin: &mut ChannelAdmin, at: usize| {
                        if let Some((role, _)) = missing.get(at) {
                            admin.add_target(OverwriteTarget::Role(*role));
                        }
                    })
                    .fill_width();
            }
            column_of(targets.iter().map(|target| {
                let target = *target;
                let selected = target == chosen;
                container(text(target_name(linked, &space, target)).label().no_wrap())
                    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                    .radius(Corner::Item)
                    .background(match selected {
                        true => palette().accent.with_alpha(0.18),
                        false => Color::TRANSPARENT,
                    })
                    .on_press(move |admin: &mut ChannelAdmin| admin.target.set(Some(target)))
                    .fill_width()
                    .any()
            }))
            .gap(Space::HAIR)
            .fill_width();
        }
        .gap(Space::SNUG)
        .width(LIST);
        column! {
            private_switch(settings);
            if synced {
                row! {
                    hint(tr!("discord.channel-admin.sync-hint"));
                    button(text(tr!("discord.channel-admin.sync")))
                        .secondary()
                        .small()
                        .on_press(|chat: &mut Chat| chat.sync_channel_permissions());
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
            }
            row! {
                list;
                container(grants(linked, settings, chosen)).grow(1.0).min_width(0.0);
            }
            .gap(Space::LOOSE)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

fn private_switch(settings: &ChannelSettings) -> Element {
    let space = settings.space;
    column! {
        field(
            tr!("discord.channel-admin.private"),
            switch(settings.is_private()).on_toggle(move |admin: &mut ChannelAdmin, on: bool| {
                let grant = match on {
                    true => Grant::Deny,
                    false => Grant::Inherit,
                };
                admin.set_grant(everyone(&space), Permissions::VIEW_CHANNEL, grant);
            }),
        );
        hint(tr!("discord.channel-admin.private-hint"));
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

fn grants(linked: &LinkedAccount, settings: &ChannelSettings, target: OverwriteTarget) -> Element {
    let space = settings.space;
    let overwrite = settings
        .overwrite(target)
        .copied()
        .unwrap_or_else(|| Overwrite::empty(target));
    let scope = Scope::of(settings.kind);
    let groups = Group::ALL.into_iter().filter_map(|group| {
        let entries = group.entries(scope);
        if entries.is_empty() {
            return None;
        }
        let rows = entries.into_iter().map(|permission| {
            let at = GRANTS
                .iter()
                .position(|grant| *grant == overwrite.state(permission))
                .unwrap_or(1);
            field(
                permission_catalog::label(permission),
                segmented(at)
                    .segment(Label::from("✕"))
                    .segment(Label::from("／"))
                    .segment(Label::from("✓"))
                    .on_select(move |admin: &mut ChannelAdmin, at: usize| {
                        admin.set_grant(target, permission, GRANTS[at.min(GRANTS.len() - 1)]);
                    }),
            )
            .any()
        });
        Some(section(group.title(), column_of(rows).gap(Space::SNUG).fill_width()).any())
    });
    column! {
        row! {
            text(target_name(linked, &space, target)).heading().semibold().grow(1.0).min_width(0.0);
            if !target.is_everyone(&space) {
                button(text(tr!("discord.channel-admin.remove-overwrite")))
                    .danger()
                    .small()
                    .on_press(move |admin: &mut ChannelAdmin| admin.remove_target(target));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        hint(tr!("discord.channel-admin.grants-hint"));
        column_of(groups).gap(Space::LOOSE).fill_width();
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

impl ChannelAdmin {
    fn add_target(&mut self, target: OverwriteTarget) {
        self.edit(|settings| {
            if settings.overwrite(target).is_none() {
                settings.overwrites.push(Overwrite::empty(target));
            }
        });
        self.target.set(Some(target));
    }

    fn remove_target(&mut self, target: OverwriteTarget) {
        self.edit(|settings| {
            settings
                .overwrites
                .retain(|overwrite| overwrite.target != target)
        });
        self.target.set(None);
    }

    fn set_grant(&mut self, target: OverwriteTarget, permission: Permissions, grant: Grant) {
        self.edit(|settings| {
            let current = settings
                .overwrite(target)
                .copied()
                .unwrap_or_else(|| Overwrite::empty(target));
            let changed = current.with(permission, grant);
            match settings
                .overwrites
                .iter()
                .position(|overwrite| overwrite.target == target)
            {
                Some(at) => settings.overwrites[at] = changed,
                None => settings.overwrites.push(changed),
            }
        });
    }
}
