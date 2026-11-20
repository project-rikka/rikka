use std::sync::Arc;

use discord_core::{Bond, Picture, Presence, Profile, Rich, Role, Space, User};
use rikka_app::prelude::*;

use super::ProfileTarget;
use crate::account::LinkedAccount;
use crate::chat::Chat;

const THEMED: f32 = 0.32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Surface {
    Popout,
    Panel,
    Full,
}

pub(super) struct Look<'a> {
    pub(super) linked: &'a LinkedAccount,
    pub(super) target: ProfileTarget,
    pub(super) surface: Surface,
    pub(super) user: User,
    pub(super) name: String,
    pub(super) face: Option<Picture>,
    pub(super) presence: Presence,
    pub(super) profile: Option<Arc<Profile>>,
    pub(super) me: bool,
    pub(super) bond: Option<Bond>,
    pub(super) roles: Vec<Role>,
    pub(super) joined: Option<i64>,
    pub(super) space: Option<Space>,
    pub(super) animate: bool,
}

impl<'a> Look<'a> {
    pub(super) fn resolve(
        chat: &'a Chat,
        target: &ProfileTarget,
        surface: Surface,
    ) -> Option<Look<'a>> {
        let linked = chat.linked(&target.key)?;
        let profile = fetched(linked, target);
        let user = linked
            .user(&target.user)
            .cloned()
            .or_else(|| profile.as_ref().map(|profile| profile.user.clone()))?;
        let space = target.space.as_ref();
        let member = space.and_then(|space| linked.member(space, &target.user));
        let local = profile.as_ref().and_then(|profile| profile.space.as_ref());
        let mut roles: Vec<Role> = space
            .map(|space| {
                let member_roles = member
                    .map(|member| &member.roles)
                    .or(local.map(|local| &local.roles));
                member_roles
                    .into_iter()
                    .flatten()
                    .filter_map(|role| linked.role(space, role).cloned())
                    .collect()
            })
            .unwrap_or_default();
        roles.sort_by_key(|role| std::cmp::Reverse(role.position));
        let face = member
            .and_then(|member| member.avatar.clone())
            .or_else(|| local.and_then(|local| local.avatar.clone()))
            .or_else(|| user.avatar.clone());
        let name = match (member, local.and_then(|local| local.nickname.as_ref())) {
            (None, Some(nickname)) => nickname.clone(),
            _ => linked.display_name(space, &user),
        };
        Some(Look {
            linked,
            target: target.clone(),
            surface,
            name,
            face,
            presence: linked.presence(&target.user).cloned().unwrap_or_default(),
            me: linked.is_me(&target.user),
            bond: linked.bond(&target.user),
            joined: member
                .and_then(|member| member.joined)
                .or_else(|| local.and_then(|local| local.joined)),
            space: space.and_then(|space| linked.space(space)).cloned(),
            roles,
            profile,
            user,
            animate: chat.appearance().animated_emoji,
        })
    }

    pub(super) fn mark(&self) -> (&ProfileTarget, Surface) {
        (&self.target, self.surface)
    }

    fn theme(&self) -> Option<u32> {
        self.profile.as_ref().and_then(|profile| profile.theme)
    }

    pub(super) fn backdrop(&self) -> Color {
        match self.theme() {
            Some(theme) => palette().surface.mix(Color::hex(theme), THEMED),
            None => palette().surface,
        }
    }

    pub(super) fn banner_color(&self) -> Color {
        let profile = self.profile.as_ref();
        self.theme()
            .or_else(|| profile.and_then(|profile| profile.accent))
            .map_or_else(|| palette().accent, Color::hex)
    }

    pub(super) fn banner(&self) -> Option<&Picture> {
        self.profile.as_ref()?.banner.as_ref()
    }

    pub(super) fn bio(&self) -> Option<&Rich> {
        self.profile
            .as_ref()
            .map(|profile| &profile.bio)
            .filter(|bio| !bio.is_empty())
    }

    pub(super) fn pronouns(&self) -> Option<&str> {
        self.profile.as_ref()?.pronouns.as_deref()
    }

    pub(super) fn created(&self) -> i64 {
        self.user.id.created_at()
    }

    pub(super) fn mutual_spaces(&self) -> Vec<&Space> {
        let Some(profile) = self.profile.as_ref() else {
            return Vec::new();
        };
        profile
            .mutual_spaces
            .iter()
            .filter_map(|space| self.linked.space(space))
            .collect()
    }

    pub(super) fn mutual_friends(&self) -> &[User] {
        self.profile
            .as_ref()
            .map_or(&[], |profile| profile.mutual_friends.as_slice())
    }
}

fn fetched(linked: &LinkedAccount, target: &ProfileTarget) -> Option<Arc<Profile>> {
    let client = linked.client().clone();
    let (user, space) = (target.user, target.space);
    let loading = fetch(
        ("discord-profile", target.key.clone(), user, space),
        move || async move { client.profile(&user, space.as_ref()).await },
    );
    match loading {
        Fetch::Ready(profile) => Some(profile),
        Fetch::Loading | Fetch::Failed(_) => None,
    }
}
