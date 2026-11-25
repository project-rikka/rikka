use std::cmp::Reverse;

use discord_core::{Permissions, ROLE_ICONS, Role, RoleEdit, RoleId, SpaceId, UserId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{Edited, SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::rgb_of;
use crate::permission_catalog::{self, Group, Scope};
use crate::widgets::avatar;

const LIST: f32 = 220.0;
const DOT: f32 = 12.0;
const LONGEST_NAME: usize = 100;
const PRESETS: [u32; 20] = [
    0x1ABC9C, 0x2ECC71, 0x3498DB, 0x9B59B6, 0xE91E63, 0xF1C40F, 0xE67E22, 0xE74C3C, 0x95A5A6,
    0x607D8B, 0x11806A, 0x1F8B4C, 0x206694, 0x71368A, 0xAD1457, 0xC27C0E, 0xA84300, 0x992D22,
    0x979C9F, 0x546E7A,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum RoleTab {
    #[default]
    Display,
    Permissions,
    Members,
}

impl RoleTab {
    const ALL: [RoleTab; 3] = [RoleTab::Display, RoleTab::Permissions, RoleTab::Members];

    fn title(self) -> Label {
        match self {
            RoleTab::Display => tr!("discord.admin.role-display"),
            RoleTab::Permissions => tr!("discord.admin.role-permissions"),
            RoleTab::Members => tr!("discord.admin.role-members"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoleDraft {
    pub(crate) id: RoleId,
    pub(crate) name: String,
    pub(crate) color: Option<u32>,
    pub(crate) hoisted: bool,
    pub(crate) mentionable: bool,
    pub(crate) permissions: Permissions,
    pub(crate) emoji: Option<String>,
}

impl RoleDraft {
    fn of(role: &Role) -> RoleDraft {
        RoleDraft {
            id: role.id,
            name: role.name.clone(),
            color: role.color,
            hoisted: role.hoisted,
            mentionable: role.mentionable,
            permissions: role.permissions,
            emoji: role.emoji.clone(),
        }
    }
}

fn edits(original: &RoleDraft, draft: &RoleDraft) -> Vec<RoleEdit> {
    let mut edits = Vec::new();
    if original.name != draft.name {
        edits.push(RoleEdit::Name(draft.name.clone()));
    }
    if original.color != draft.color {
        edits.push(RoleEdit::Color(draft.color));
    }
    if original.hoisted != draft.hoisted {
        edits.push(RoleEdit::Hoist(draft.hoisted));
    }
    if original.mentionable != draft.mentionable {
        edits.push(RoleEdit::Mentionable(draft.mentionable));
    }
    if original.permissions != draft.permissions {
        edits.push(RoleEdit::Permissions(draft.permissions));
    }
    if original.emoji != draft.emoji {
        edits.push(RoleEdit::Emoji(draft.emoji.clone()));
    }
    edits
}

fn ordered<'a>(linked: &'a LinkedAccount, space: &SpaceId) -> Vec<&'a Role> {
    let mut roles: Vec<&Role> = linked.roles(space).iter().collect();
    roles.sort_by_key(|role| Reverse((role.position, Reverse(role.id))));
    roles
}

fn ceiling(linked: &LinkedAccount, space: &SpaceId) -> Option<i32> {
    let me = linked.me()?.id;
    match linked.space(space)?.owned_by(&me) {
        true => None,
        false => Some(linked.top_role_position(space, &me)),
    }
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let roles = ordered(linked, space);
    let ceiling = ceiling(linked, space);
    let editable = |role: &Role| ceiling.is_none_or(|top| role.position < top);
    let selected = admin.role.as_ref().map(|edited| edited.draft.id);
    let movable: Vec<RoleId> = roles
        .iter()
        .filter(|role| !role.is_everyone(space) && editable(role))
        .map(|role| role.id)
        .collect();
    let list = column! {
        button(text(tr!("discord.admin.create-role")))
            .primary()
            .small()
            .on_press(|chat: &mut Chat| chat.create_role())
            .fill_width();
        column_of(roles.iter().map(|role| {
            let count = match admin.role_counts.as_ref() {
                Some(Ok(counts)) => counts.get(&role.id).copied(),
                _ => None,
            };
            let position = movable.iter().position(|id| *id == role.id);
            role_row(role, selected == Some(role.id), count, position, &movable)
        }))
        .gap(Space::HAIR)
        .fill_width();
    }
    .gap(Space::SNUG)
    .width(LIST);
    let editor = match admin.role.as_ref() {
        Some(edited) => editor(
            admin,
            linked,
            space,
            edited,
            roles
                .iter()
                .any(|role| role.id == edited.draft.id && editable(role)),
        ),
        None => hint(tr!("discord.admin.pick-role")),
    };
    row! {
        list;
        container(editor).grow(1.0).min_width(0.0);
    }
    .gap(Space::LOOSE)
    .fill_width()
    .any()
}

fn role_row(
    role: &Role,
    selected: bool,
    count: Option<u32>,
    position: Option<usize>,
    movable: &[RoleId],
) -> Element {
    let tint = role.color.map_or_else(|| palette().text_muted, Color::hex);
    let id = role.id;
    let order = movable.to_vec();
    let up = position.filter(|at| *at > 0);
    let down = position.filter(|at| at + 1 < movable.len());
    let (raise, lower) = (order.clone(), order);
    container(
        row! {
            container(spacer()).background(tint).radius(DOT / 2.0).size(Length::px(DOT), Length::px(DOT));
            text(role.name.clone()).label().no_wrap().grow(1.0).min_width(0.0);
            if let Some(count) = count {
                text(count.to_string()).caption().muted();
            }
            if let Some(at) = up {
                button(icon(MaterialSymbol::ARROW_UPWARD).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.swap_roles(&raise, at, at - 1));
            }
            if let Some(at) = down {
                button(icon(MaterialSymbol::ARROW_DOWNWARD).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.swap_roles(&lower, at, at + 1));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .background(match selected {
        true => palette().accent.with_alpha(0.18),
        false => Color::TRANSPARENT,
    })
    .on_press(move |chat: &mut Chat| chat.select_role(&id))
    .fill_width()
    .any()
}

fn editor(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    edited: &Edited<RoleDraft>,
    editable: bool,
) -> Element {
    let draft = &edited.draft;
    let role = linked.role(space, &draft.id);
    let everyone = role.is_some_and(|role| role.is_everyone(space));
    let managed = role.is_some_and(|role| role.managed);
    let tabs = match everyone {
        true => vec![RoleTab::Permissions],
        false => RoleTab::ALL.to_vec(),
    };
    let tab = match everyone {
        true => RoleTab::Permissions,
        false => *admin.role_tab,
    };
    let body = match tab {
        RoleTab::Display => display(admin, linked, space, draft, editable && !managed),
        RoleTab::Permissions => permissions(draft, editable),
        RoleTab::Members => members(admin, linked, space, draft.id, editable && !managed),
    };
    let at = tabs.iter().position(|known| *known == tab).unwrap_or(0);
    column! {
        row! {
            text(draft.name.clone()).heading().semibold().grow(1.0).min_width(0.0);
            if !everyone && !managed && editable {
                button(text(tr!("discord.admin.delete-role")))
                    .danger()
                    .small()
                    .on_press(|chat: &mut Chat| chat.ask_delete_role());
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if !editable {
            hint(tr!("discord.admin.role-above"));
        }
        tabs.iter()
            .fold(segmented(at), |choice, tab| choice.segment(tab.title()))
            .on_select(move |admin: &mut SpaceAdmin, at: usize| {
                if let Some(tab) = tabs.get(at) {
                    admin.role_tab.set(*tab);
                }
            })
            .fill_width();
        body;
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn display(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    draft: &RoleDraft,
    editable: bool,
) -> Element {
    let shown = draft.color.map_or_else(|| palette().text_muted, Color::hex);
    let icons = linked
        .space(space)
        .is_some_and(|known| known.has(ROLE_ICONS));
    column! {
        text(tr!("discord.admin.role-name")).caption().semibold().muted();
        text_field(&admin.docs.role_name)
            .max_len(LONGEST_NAME)
            .enabled(editable)
            .on_change(|admin: &mut SpaceAdmin| {
                let name = admin.docs.role_name.text();
                admin.edit_role(|draft| draft.name = name);
            })
            .fill_width();
        text(tr!("discord.admin.role-color")).caption().semibold().muted();
        row! {
            color_well(shown).enabled(editable).on_commit(|admin: &mut SpaceAdmin, picked: Color| {
                admin.edit_role(|draft| draft.color = Some(rgb_of(picked)));
            });
            button(text(tr!("discord.manage.color-default")))
                .ghost()
                .small()
                .enabled(editable && draft.color.is_some())
                .on_press(|admin: &mut SpaceAdmin| admin.edit_role(|draft| draft.color = None));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        grid_of(10, PRESETS.map(|preset| {
            container(spacer())
                .background(Color::hex(preset))
                .radius(Corner::Item)
                .size(Length::px(24.0), Length::px(24.0))
                .on_press(move |admin: &mut SpaceAdmin| {
                    if editable {
                        admin.edit_role(|draft| draft.color = Some(preset));
                    }
                })
                .any()
        }))
        .gap(Space::TIGHT);
        field(
            tr!("discord.admin.role-hoist"),
            switch(draft.hoisted).enabled(editable).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                admin.edit_role(|draft| draft.hoisted = on);
            }),
        );
        field(
            tr!("discord.admin.role-mentionable"),
            switch(draft.mentionable).enabled(editable).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                admin.edit_role(|draft| draft.mentionable = on);
            }),
        );
        if icons {
            text(tr!("discord.admin.role-emoji")).caption().semibold().muted();
            text_field(&admin.docs.role_emoji)
                .enabled(editable)
                .max_len(16)
                .on_change(|admin: &mut SpaceAdmin| {
                    let emoji = Some(admin.docs.role_emoji.text().trim().to_owned())
                        .filter(|emoji| !emoji.is_empty());
                    admin.edit_role(|draft| draft.emoji = emoji);
                })
                .width(120.0);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn permissions(draft: &RoleDraft, editable: bool) -> Element {
    let groups = Group::ALL.into_iter().map(|group| {
        let rows = group.entries(Scope::Space).into_iter().map(|permission| {
            field(
                permission_catalog::label(permission),
                switch(draft.permissions.contains(permission))
                    .enabled(editable)
                    .on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
                        admin.edit_role(|draft| {
                            draft.permissions = draft.permissions.with(permission, on);
                        });
                    }),
            )
            .any()
        });
        section(group.title(), column_of(rows).gap(Space::SNUG).fill_width()).any()
    });
    column! {
        button(text(tr!("discord.admin.clear-permissions")))
            .ghost()
            .small()
            .enabled(editable && !draft.permissions.is_empty())
            .on_press(|admin: &mut SpaceAdmin| {
                admin.edit_role(|draft| draft.permissions = Permissions::NONE);
            });
        column_of(groups).gap(Space::LOOSE).fill_width();
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn members(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    role: RoleId,
    editable: bool,
) -> Element {
    let space = *space;
    let listed = loading(&admin.role_members, |users| {
        column_of(users.iter().map(|user| {
            let name = linked.name_of(Some(&space), user).unwrap_or_else(|| user.to_string());
            let picture = linked.user(user).and_then(|known| known.avatar.clone());
            let removed = *user;
            row! {
                avatar(picture.as_ref(), &name, 28.0);
                text(name.clone()).label().grow(1.0).min_width(0.0);
                if editable {
                    button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                        .ghost()
                        .small()
                        .on_press(move |chat: &mut Chat| chat.change_role_member(&role, &removed, false));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .any()
        }))
        .gap(Space::TIGHT)
        .fill_width()
        .any()
    });
    let found = admin.members.as_ref().map(|_| loading(&admin.members, |page| {
        let held: Vec<UserId> = match admin.role_members.as_ref() {
            Some(Ok(users)) => users.clone(),
            _ => Vec::new(),
        };
        column_of(
            page.hits
                .iter()
                .filter(|hit| !held.contains(&hit.user.id))
                .take(10)
                .map(|hit| {
                    let added = hit.user.id;
                    row! {
                        avatar(hit.user.avatar.as_ref(), hit.user.name(), 24.0);
                        text(hit.user.name().to_owned()).label().grow(1.0).min_width(0.0);
                        button(text(tr!("discord.admin.add")))
                            .secondary()
                            .small()
                            .on_press(move |chat: &mut Chat| chat.change_role_member(&role, &added, true));
                    }
                    .gap(Space::SNUG)
                    .align_items(Align::Center)
                    .fill_width()
                    .any()
                }),
        )
        .gap(Space::TIGHT)
        .fill_width()
        .any()
    }));
    column! {
        if editable {
            text_field(&admin.docs.search)
                .placeholder(tr!("discord.admin.search-members"))
                .on_submit(|chat: &mut Chat| chat.search_admin_members())
                .fill_width();
            if let Some(found) = found {
                found;
            }
        }
        listed;
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

impl SpaceAdmin {
    pub(super) fn edit_role(&mut self, change: impl FnOnce(&mut RoleDraft)) {
        if let Some(mut edited) = (*self.role).clone() {
            change(&mut edited.draft);
            self.role.set(Some(edited));
            self.warned.set(false);
        }
    }

    fn selected_role(&self) -> Option<RoleId> {
        self.role.as_ref().map(|edited| edited.draft.id)
    }

    fn select(&mut self, role: &Role) {
        self.docs.role_name.reset(&role.name);
        self.docs
            .role_emoji
            .reset(role.emoji.as_deref().unwrap_or_default());
        self.role.set(Some(Edited::new(RoleDraft::of(role))));
        self.role_members.set(None);
    }

    pub(super) fn reset_role(&mut self) {
        if let Some(edited) = (*self.role).clone() {
            self.docs.role_name.reset(&edited.original.name);
            self.docs
                .role_emoji
                .reset(edited.original.emoji.as_deref().unwrap_or_default());
            self.role.set(Some(Edited::new(edited.original)));
        }
    }
}

impl Chat {
    pub(super) fn load_role_counts(&mut self) {
        self.fetch(
            |client, space| async move { client.role_member_counts(&space).await },
            |admin, counts| admin.role_counts.set(Some(counts)),
        );
    }

    fn select_role(&mut self, role: &RoleId) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        if self.space_admin().is_dirty() {
            self.space_admin_mut().warn();
            return;
        }
        let Some(found) = self
            .linked(&key)
            .and_then(|linked| linked.role(&space, role))
            .cloned()
        else {
            return;
        };
        self.open_role(&found);
    }

    fn open_role(&mut self, role: &Role) {
        let Some((_, space)) = self.admin_target() else {
            return;
        };
        self.space_admin_mut().select(role);
        if role.is_everyone(&space) {
            return;
        }
        let role = role.id;
        self.fetch(
            move |client, space| async move { client.role_members(&space, &role).await },
            move |admin, users| {
                if admin.selected_role() == Some(role) {
                    admin.role_members.set(Some(users));
                }
            },
        );
    }

    fn create_role(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let name = tr!("discord.admin.new-role").to_string();
        self.act(
            &key,
            move |client| {
                let name = name.clone();
                async move { client.create_role(&space, &name).await }
            },
            move |chat: &mut Chat, role: Role| {
                if chat.admin_for(space).is_some() && !chat.space_admin().is_dirty() {
                    chat.open_role(&role);
                }
            },
        );
    }

    fn swap_roles(&mut self, order: &[RoleId], from: usize, to: usize) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let (Some(first), Some(second)) = (order.get(from).copied(), order.get(to).copied()) else {
            return;
        };
        let positions = self.linked(&key).and_then(|linked| {
            Some((
                linked.role(&space, &first)?.position,
                linked.role(&space, &second)?.position,
            ))
        });
        let Some((first_at, second_at)) = positions else {
            return;
        };
        let moves = vec![(first, second_at), (second, first_at)];
        self.act(
            &key,
            move |client| {
                let moves = moves.clone();
                async move { client.move_roles(&space, moves).await }
            },
            |_: &mut Chat, ()| {},
        );
    }

    pub(super) fn save_role(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(edited) = self.space_admin().role.as_ref().cloned() else {
            return;
        };
        let changes = edits(&edited.original, &edited.draft);
        let sent = edited.draft;
        self.space_admin_mut().saving.set(true);
        self.attempt(
            &key,
            move |client| {
                let (changes, role) = (changes.clone(), sent.id);
                async move { client.edit_role(&space, &role, changes).await }
            },
            move |chat: &mut Chat, saved: Option<()>| {
                chat.admin_saved(space);
                if saved.is_some()
                    && let Some(admin) = chat.admin_for(space)
                    && let Some(mut edited) = (*admin.role).clone()
                    && edited.draft.id == sent.id
                {
                    edited.original = sent.clone();
                    admin.role.set(Some(edited));
                }
            },
        );
    }

    fn ask_delete_role(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(edited) = self.space_admin().role.as_ref().cloned() else {
            return;
        };
        let role = edited.original.id;
        self.confirm(
            crate::manage::Prompt {
                title: tr!("discord.admin.delete-role-title").to_string(),
                body: tr!(
                    "discord.admin.delete-role-body",
                    name = edited.original.name.as_str()
                )
                .to_string(),
                accept: tr!("discord.manage.delete").to_string(),
            },
            move |chat: &mut Chat| {
                chat.act(
                    &key,
                    move |client| async move { client.delete_role(&space, &role).await },
                    move |chat: &mut Chat, ()| {
                        if let Some(admin) = chat.admin_for(space)
                            && admin.selected_role() == Some(role)
                        {
                            admin.role.set(None);
                        }
                    },
                );
            },
        );
    }

    fn change_role_member(&mut self, role: &RoleId, user: &UserId, granted: bool) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let (role, user) = (*role, *user);
        self.act(
            &key,
            move |client| async move { client.grant_role(&space, &user, &role, granted).await },
            move |chat: &mut Chat, ()| {
                if let Some(admin) = chat.admin_for(space)
                    && admin.selected_role() == Some(role)
                    && let Some(Ok(mut users)) = (*admin.role_members).clone()
                {
                    users.retain(|known| *known != user);
                    if granted {
                        users.push(user);
                    }
                    admin.role_members.set(Some(Ok(users)));
                }
            },
        );
    }
}
