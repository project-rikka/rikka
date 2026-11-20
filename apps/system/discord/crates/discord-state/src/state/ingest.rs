use std::collections::HashMap;

use discord_core::{
    ChannelId, Emoji, Member, Presence, Ready, Ringing, SpaceId, Sticker, Unread, User, UserId,
    VoiceState,
};
use serde::de::DeserializeOwned;

use super::reads::Reads;
use super::{Guild, Outcome, Pending, State};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::Preloaded;
use discord_wire::snowflake;

pub(super) fn parse<T: DeserializeOwned>(event: &str, data: &str) -> Option<T> {
    match serde_json::from_str(data) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            tracing::warn!(%error, event, "skipping unreadable Discord event");
            None
        }
    }
}

impl State {
    pub(super) fn ready(&mut self, data: &str) {
        let Some(ready) = parse::<wire::Ready>("READY", data) else {
            return;
        };
        let focus = std::mem::take(&mut self.focus);
        let ranges = std::mem::take(&mut self.ranges);
        *self = State {
            focus,
            ranges,
            ..State::new(self.now)
        };
        self.me = ready.user.clone();
        self.settings = ready
            .user_settings_proto
            .as_deref()
            .map(Preloaded::decode_base64)
            .unwrap_or_default();
        self.notifications = ready
            .user_guild_settings
            .iter()
            .map(|settings| (settings.guild_id.clone(), settings.clone()))
            .collect();
        self.reads = Reads::load(&ready.read_state);
        self.blocked = ready
            .relationships
            .iter()
            .filter(|relationship| relationship.kind == 2)
            .map(|relationship| relationship.user_id().to_owned())
            .collect();
        self.sessions = ready.sessions.clone();
        let me = self.me.id.clone();
        for (index, guild) in ready.guilds.iter().enumerate() {
            let mine = ready
                .merged_members
                .get(index)
                .and_then(|members| {
                    members
                        .iter()
                        .find(|member| member.user_id() == Some(me.as_str()))
                })
                .or_else(|| {
                    guild
                        .members
                        .iter()
                        .find(|member| member.user_id() == Some(me.as_str()))
                });
            self.ingest_guild(guild, mine);
        }
        for channel in &ready.private_channels {
            self.track(channel.clone(), None, false);
        }
        self.pending = Some(Pending {
            ready,
            buffered: Vec::new(),
            since: self.now.unix_ms,
        });
        self.ticked = self.now.unix_ms;
    }

    pub(super) fn ingest_guild(&mut self, guild: &wire::Guild, mine: Option<&wire::Member>) {
        let previous = self.guilds.remove(&guild.id);
        let known = Guild {
            properties: guild.properties(),
            roles: guild.roles.clone(),
            my_roles: mine.map(|member| member.roles.clone()).unwrap_or_default(),
            timeout_until: mine.and_then(|member| {
                translate::instant(member.communication_disabled_until.as_deref())
            }),
            joined_at: mine
                .and_then(|member| translate::instant(member.joined_at.as_deref()))
                .or_else(|| translate::instant(guild.joined_at.as_deref())),
            unavailable: guild.unavailable,
            shown: previous.and_then(|old| old.shown),
        };
        self.guilds.insert(guild.id.clone(), known);
        for channel in &guild.channels {
            self.track(channel.clone(), Some(guild.id.clone()), false);
        }
        for thread in &guild.threads {
            self.track(
                thread.clone(),
                Some(guild.id.clone()),
                thread.member.is_some(),
            );
        }
        self.known_members
            .entry(guild.id.clone())
            .or_default()
            .insert(self.me.id.clone());
    }

    pub(super) fn supplemental(&mut self, data: &str, outcome: &mut Outcome) {
        let supplemental =
            parse::<wire::Supplemental>("READY_SUPPLEMENTAL", data).unwrap_or_default();
        let Some(Pending {
            ready, buffered, ..
        }) = self.pending.take()
        else {
            return;
        };
        for (event, data) in &buffered {
            if event == "CALL_CREATE"
                && let Some(call) = parse::<wire::Call>(event, data)
            {
                self.calls.insert(call.channel_id.clone(), ringing(&call));
            }
        }
        let built = self.compose(&ready, &supplemental);
        for space in &built.spaces {
            if let Some(known) = self.guilds.get_mut(&space.id.to_string()) {
                known.shown = Some(space.clone());
            }
        }
        for channel in &built.channels {
            if let Some(tracked) = self.channels.get_mut(&channel.id.to_string()) {
                tracked.shown = Some(channel.clone());
            }
        }
        outcome.update(discord_core::Update::Ready(Box::new(built)));
        self.subscribe(outcome);
        for (event, data) in buffered {
            self.route(&event, &data, outcome);
        }
    }

    fn compose(&self, ready: &wire::Ready, supplemental: &wire::Supplemental) -> Ready {
        let mut users: HashMap<String, User> = HashMap::new();
        let mut adopt = |wire: &wire::User| {
            if wire.is_full() {
                users.insert(wire.id.clone(), translate::user(wire));
            }
        };
        adopt(&ready.user);
        ready.users.iter().for_each(&mut adopt);
        ready
            .relationships
            .iter()
            .filter_map(|relationship| relationship.user.as_ref())
            .for_each(&mut adopt);
        ready
            .private_channels
            .iter()
            .flat_map(|channel| &channel.recipients)
            .for_each(&mut adopt);
        let mut members: Vec<Member> = Vec::new();
        let mut presences: Vec<(UserId, Presence)> = Vec::new();
        let mut voice: Vec<VoiceState> = Vec::new();
        for (index, guild) in ready.guilds.iter().enumerate() {
            let listed = ready
                .merged_members
                .get(index)
                .into_iter()
                .chain(supplemental.merged_members.get(index))
                .flatten()
                .chain(&guild.members);
            for member in listed {
                if let Some(user) = &member.user {
                    adopt(user);
                }
                members.extend(translate::member(&guild.id, member));
            }
            let guild_presences = supplemental
                .merged_presences
                .guilds
                .get(index)
                .into_iter()
                .flatten()
                .chain(&guild.presences);
            for presence in guild_presences {
                if let Some(user) = presence.user.as_ref() {
                    adopt(user);
                }
                if let Some(user) = presence.user_id() {
                    presences.push((snowflake::id::<UserId>(user), presence_of(presence)));
                }
            }
            voice.extend(
                guild
                    .voice_states
                    .iter()
                    .filter_map(|state| voice_in(state, &guild.id)),
            );
        }
        for guild in &supplemental.guilds {
            voice.extend(
                guild
                    .voice_states
                    .iter()
                    .filter_map(|state| voice_in(state, &guild.id)),
            );
        }
        for presence in &supplemental.merged_presences.friends {
            if let Some(user) = presence.user.as_ref() {
                adopt(user);
            }
            if let Some(user) = presence.user_id() {
                presences.push((snowflake::id::<UserId>(user), presence_of(presence)));
            }
        }
        let channels: Vec<discord_core::Channel> = self
            .channels
            .keys()
            .filter_map(|id| self.channel_now(id))
            .collect();
        let unread: Vec<Unread> = self
            .channels
            .iter()
            .filter(|(_, tracked)| {
                tracked.wire.last_message_id.is_some() || self.reads.has_mentions(&tracked.wire.id)
            })
            .map(|(id, tracked)| self.reads.unread(&self.activity(id, tracked)))
            .collect();
        let mut emojis: Vec<(SpaceId, Vec<Emoji>)> = Vec::new();
        let mut stickers: Vec<(SpaceId, Vec<Sticker>)> = Vec::new();
        let mut roles = Vec::new();
        for guild in &ready.guilds {
            let space = snowflake::id::<SpaceId>(guild.id.as_str());
            roles.push((space, guild.roles.iter().map(translate::role).collect()));
            emojis.push((
                space,
                guild
                    .emojis
                    .iter()
                    .filter_map(translate::custom_emoji)
                    .collect(),
            ));
            stickers.push((
                space,
                guild
                    .stickers
                    .iter()
                    .map(|sticker| {
                        translate::sticker(
                            &sticker.id,
                            &sticker.name,
                            sticker.format_type,
                            sticker.description.as_deref(),
                        )
                    })
                    .collect(),
            ));
        }
        Ready {
            me: translate::user(&ready.user),
            spaces: ready
                .guilds
                .iter()
                .filter_map(|guild| self.space_now(&guild.id))
                .collect(),
            folders: self.settings.folders(),
            roles,
            emojis,
            stickers,
            channels,
            users: users.into_values().collect(),
            members,
            presences,
            friends: ready
                .relationships
                .iter()
                .filter_map(translate::friend)
                .collect(),
            unread,
            voice,
            calls: self.calls.values().cloned().collect(),
            own_presence: self.own_presence(self.now),
        }
    }
}

pub(super) fn presence_of(wire: &wire::Presence) -> Presence {
    translate::presence(
        wire.status.as_deref(),
        &wire.activities,
        wire.client_status.as_ref(),
    )
}

pub(super) fn voice_in(state: &wire::VoiceState, guild: &str) -> Option<VoiceState> {
    let mut state = state.clone();
    if state.guild_id.is_none() {
        state.guild_id = Some(guild.to_owned());
    }
    translate::voice_state(&state)
}

pub(super) fn ringing(call: &wire::Call) -> Ringing {
    Ringing {
        channel: snowflake::id::<ChannelId>(call.channel_id.as_str()),
        ringing: call
            .ringing
            .iter()
            .map(|user| snowflake::id::<UserId>(user.as_str()))
            .collect(),
    }
}
