use discord_core::{Abilities, Channel, ChannelId, Space, Update};

use super::notify::{self, Arrival};
use super::permissions::{self, Viewer};
use super::{Outcome, State, Tracked};
use discord_translate::{self as translate, Reader};
use discord_wire as wire;
use discord_wire::snowflake;
use discord_wire::{ChannelOverride, MuteConfig, notification_level};

const DEFAULT_LEVEL: u8 = notification_level::ALL;

pub(super) fn silenced(muted: bool, config: Option<&MuteConfig>, now: i64) -> bool {
    muted
        && config
            .and_then(|config| translate::instant(config.end_time.as_deref()))
            .is_none_or(|end| end > now)
}

impl State {
    pub(super) fn track(&mut self, wire: wire::Channel, guild: Option<String>, joined: bool) {
        let shown = self.channels.remove(&wire.id).and_then(|old| old.shown);
        self.channels.insert(
            wire.id.clone(),
            Tracked {
                wire,
                guild,
                joined,
                shown,
            },
        );
    }

    pub(super) fn channel_now(&self, id: &str) -> Option<Channel> {
        let tracked = self.channels.get(id)?;
        let looks = translate::Looks {
            abilities: self.abilities(tracked),
            muted: self.override_muted(tracked, self.now.unix_ms),
            joined: tracked.joined,
        };
        translate::channel(&tracked.wire, tracked.guild.as_deref(), looks)
    }

    pub(super) fn channel_out(&mut self, id: &str, force: bool, outcome: &mut Outcome) {
        let Some(now) = self.channel_now(id) else {
            return;
        };
        let Some(tracked) = self.channels.get_mut(id) else {
            return;
        };
        if force || tracked.shown.as_ref() != Some(&now) {
            tracked.shown = Some(now.clone());
            outcome.update(Update::Channel(now));
        }
    }

    pub(super) fn refresh(&mut self, guild: Option<&str>, outcome: &mut Outcome) {
        let ids: Vec<String> = self
            .channels
            .iter()
            .filter(|(_, tracked)| guild.is_none() || tracked.guild.as_deref() == guild)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.channel_out(&id, false, outcome);
        }
    }

    pub(super) fn space_now(&self, guild: &str) -> Option<Space> {
        let known = self.guilds.get(guild)?;
        Some(translate::space(
            guild,
            &known.properties,
            self.space_muted(guild, self.now.unix_ms),
            known.unavailable,
        ))
    }

    pub(super) fn space_out(&mut self, guild: &str, force: bool, outcome: &mut Outcome) {
        let Some(now) = self.space_now(guild) else {
            return;
        };
        let Some(known) = self.guilds.get_mut(guild) else {
            return;
        };
        if force || known.shown.as_ref() != Some(&now) {
            known.shown = Some(now.clone());
            outcome.update(Update::Space(now));
        }
    }

    pub(super) fn space_muted(&self, guild: &str, now: i64) -> bool {
        self.notifications
            .get(&Some(guild.to_owned()))
            .is_some_and(|settings| silenced(settings.muted, settings.mute_config.as_ref(), now))
    }

    fn override_of(&self, guild: Option<&str>, channel: &str) -> Option<&ChannelOverride> {
        self.notifications
            .get(&guild.map(str::to_owned))?
            .channel_overrides
            .iter()
            .find(|entry| entry.channel_id == channel)
    }

    fn override_muted(&self, tracked: &Tracked, now: i64) -> bool {
        self.override_of(tracked.guild.as_deref(), &tracked.wire.id)
            .is_some_and(|entry| silenced(entry.muted, entry.mute_config.as_ref(), now))
    }

    fn lineage(&self, tracked: &Tracked) -> Vec<String> {
        let mut chain = vec![tracked.wire.id.clone()];
        let mut parent = tracked.wire.parent_id.clone();
        while let Some(id) = parent {
            if chain.contains(&id) || chain.len() > 3 {
                break;
            }
            parent = self
                .channels
                .get(&id)
                .and_then(|known| known.wire.parent_id.clone());
            chain.push(id);
        }
        chain
    }

    pub(super) fn muted_anywhere(&self, tracked: &Tracked, now: i64) -> bool {
        let guild = tracked.guild.as_deref();
        guild.is_some_and(|guild| self.space_muted(guild, now))
            || self.lineage(tracked).iter().any(|id| {
                self.override_of(guild, id)
                    .is_some_and(|entry| silenced(entry.muted, entry.mute_config.as_ref(), now))
            })
    }

    pub(super) fn level(&self, tracked: &Tracked) -> u8 {
        let Some(guild) = tracked.guild.as_deref() else {
            return notification_level::ALL;
        };
        let mut levels: Vec<u8> = self
            .lineage(tracked)
            .iter()
            .map(|id| {
                self.override_of(Some(guild), id)
                    .map_or(notification_level::INHERIT, |entry| {
                        entry.message_notifications
                    })
            })
            .collect();
        levels.push(
            self.notifications
                .get(&Some(guild.to_owned()))
                .map_or(notification_level::INHERIT, |settings| {
                    settings.message_notifications
                }),
        );
        let fallback = self
            .guilds
            .get(guild)
            .and_then(|known| known.properties.default_message_notifications)
            .unwrap_or(DEFAULT_LEVEL);
        let resolved = notify::resolve(&levels, fallback);
        if tracked.wire.is_thread() && !tracked.joined && resolved == notification_level::ALL {
            notification_level::MENTIONS
        } else {
            resolved
        }
    }

    pub(super) fn reader(&self, guild: Option<&str>) -> Reader<'_> {
        let settings = self.notifications.get(&guild.map(str::to_owned));
        Reader {
            me: &self.me.id,
            roles: guild
                .and_then(|guild| self.guilds.get(guild))
                .map_or(&[], |known| known.my_roles.as_slice()),
            suppress_everyone: settings.is_some_and(|settings| settings.suppress_everyone),
            suppress_roles: settings.is_some_and(|settings| settings.suppress_roles),
        }
    }

    pub(super) fn arrival(&self, wire: &wire::Message, tracked: Option<&Tracked>) -> Arrival {
        let guild = wire
            .guild_id
            .clone()
            .or_else(|| tracked.and_then(|tracked| tracked.guild.clone()));
        let reader = self.reader(guild.as_deref());
        let author = wire.author.as_ref().map_or("", |author| author.id.as_str());
        let now = self.now.unix_ms;
        Arrival {
            from_me: author == self.me.id,
            from_blocked: self.blocked.contains(author),
            silent: wire.flags & wire::message_flags::SUPPRESS_NOTIFICATIONS != 0,
            private: guild.is_none(),
            busy: self.status() == discord_core::Status::Busy,
            muted: tracked.is_some_and(|tracked| self.muted_anywhere(tracked, now)),
            level: tracked.map_or(notification_level::ALL, |tracked| self.level(tracked)),
            direct: reader.directly(wire),
            everyone: wire.mention_everyone,
            role: reader.by_role(wire),
            suppress_everyone: reader.suppress_everyone,
            suppress_roles: reader.suppress_roles,
        }
    }

    fn abilities(&self, tracked: &Tracked) -> Abilities {
        let Some(guild_id) = tracked.guild.as_deref() else {
            return permissions::private();
        };
        let Some(guild) = self.guilds.get(guild_id) else {
            return Abilities::NONE;
        };
        let viewer = Viewer {
            user: &self.me.id,
            guild: guild_id,
            owner: guild.properties.owner_id.as_deref(),
            roles: &guild.my_roles,
            timed_out: guild
                .timeout_until
                .is_some_and(|until| until > self.now.unix_ms),
        };
        let base = permissions::base(&viewer, &guild.roles);
        let thread = tracked.wire.is_thread();
        let source = if thread {
            tracked
                .wire
                .parent_id
                .as_deref()
                .and_then(|parent| self.channels.get(parent))
                .map_or(&tracked.wire, |parent| &parent.wire)
        } else {
            &tracked.wire
        };
        let allowed = permissions::in_channel(&viewer, base, &source.permission_overwrites);
        let locked = thread.then(|| {
            tracked
                .wire
                .thread_metadata
                .as_ref()
                .is_some_and(|metadata| metadata.locked)
        });
        permissions::abilities(allowed, locked)
    }

    pub(super) fn forget_channel(&mut self, id: &str, outcome: &mut Outcome) {
        if self.channels.remove(id).is_some() {
            outcome.update(Update::ChannelGone(snowflake::id::<ChannelId>(id)));
        }
    }
}
