use std::sync::Arc;

use discord_core::{
    ChannelId, MessageId, Notice, ReactionChange, ReactionKind, SpaceId, Update, UserId,
};

use super::ingest::{parse, presence_of, ringing, voice_in};
use super::{Outcome, Signal, State, StreamSignal};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::Preloaded;
use discord_wire::snowflake;

impl State {
    pub(super) fn route(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        match event {
            "GUILD_CREATE" => self.guild_create(data, outcome),
            "GUILD_UPDATE" => self.guild_update(data, outcome),
            "GUILD_DELETE" => self.guild_delete(data, outcome),
            "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "THREAD_CREATE" | "THREAD_UPDATE" => {
                self.channel_upsert(event, data, outcome);
            }
            "CHANNEL_DELETE" | "THREAD_DELETE" => {
                if let Some(removed) = parse::<wire::Channel>(event, data) {
                    self.forget_channel(&removed.id, outcome);
                }
            }
            "THREAD_LIST_SYNC" => self.thread_sync(data, outcome),
            "THREAD_MEMBER_UPDATE" => self.thread_member(data, outcome),
            "THREAD_MEMBERS_UPDATE" => self.thread_members(data, outcome),
            "CHANNEL_RECIPIENT_ADD" | "CHANNEL_RECIPIENT_REMOVE" => {
                self.recipient(event, data, outcome)
            }
            "GUILD_ROLE_CREATE" | "GUILD_ROLE_UPDATE" => self.role_upsert(event, data, outcome),
            "GUILD_ROLE_DELETE" => self.role_delete(data, outcome),
            "GUILD_MEMBER_ADD" | "GUILD_MEMBER_UPDATE" => {
                if let Some(member) = parse::<wire::Member>(event, data)
                    && let Some(guild) = member.guild_id.clone()
                {
                    self.members_out(&guild, std::slice::from_ref(&member), outcome);
                }
            }
            "GUILD_MEMBER_REMOVE" => {
                if let Some(removed) = parse::<wire::MemberRemove>(event, data) {
                    if let Some(known) = self.known_members.get_mut(&removed.guild_id) {
                        known.remove(&removed.user.id);
                    }
                    outcome.update(Update::MemberGone {
                        space: snowflake::id::<SpaceId>(removed.guild_id.as_str()),
                        user: snowflake::id::<UserId>(removed.user.id.as_str()),
                    });
                }
            }
            "GUILD_MEMBERS_CHUNK" => {
                if let Some(chunk) = parse::<wire::MembersChunk>(event, data) {
                    self.members_out(&chunk.guild_id, &chunk.members, outcome);
                    for presence in &chunk.presences {
                        self.presence_out(presence, outcome);
                    }
                }
            }
            "GUILD_EMOJIS_UPDATE" => {
                if let Some(update) = parse::<wire::GuildEmojis>(event, data) {
                    outcome.update(Update::Emojis {
                        space: snowflake::id::<SpaceId>(update.guild_id.as_str()),
                        emojis: update
                            .emojis
                            .iter()
                            .filter_map(translate::custom_emoji)
                            .collect(),
                    });
                }
            }
            "GUILD_STICKERS_UPDATE" => {
                if let Some(update) = parse::<wire::GuildStickers>(event, data) {
                    outcome.update(Update::Stickers {
                        space: snowflake::id::<SpaceId>(update.guild_id.as_str()),
                        stickers: update
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
                    });
                }
            }
            "MESSAGE_CREATE" => self.message_create(data, outcome),
            "MESSAGE_UPDATE" => self.message_update(data, outcome),
            "MESSAGE_DELETE" => {
                if let Some(removed) = parse::<wire::MessageDelete>(event, data) {
                    outcome.update(Update::Deleted {
                        channel: snowflake::id::<ChannelId>(removed.channel_id.as_str()),
                        messages: vec![snowflake::id::<MessageId>(removed.id.as_str())],
                    });
                }
            }
            "MESSAGE_DELETE_BULK" => {
                if let Some(removed) = parse::<wire::MessageDeleteBulk>(event, data) {
                    outcome.update(Update::Deleted {
                        channel: snowflake::id::<ChannelId>(removed.channel_id.as_str()),
                        messages: removed
                            .ids
                            .iter()
                            .map(|id| snowflake::id::<MessageId>(id.as_str()))
                            .collect(),
                    });
                }
            }
            "MESSAGE_REACTION_ADD"
            | "MESSAGE_REACTION_REMOVE"
            | "MESSAGE_REACTION_REMOVE_ALL"
            | "MESSAGE_REACTION_REMOVE_EMOJI" => self.reaction(event, data, outcome),
            "MESSAGE_REACTION_ADD_MANY" => self.reactions(data, outcome),
            "TYPING_START" => self.typing(data, outcome),
            "PRESENCE_UPDATE" => {
                if let Some(presence) = parse::<wire::Presence>(event, data) {
                    self.presence_out(&presence, outcome);
                }
            }
            "SESSIONS_REPLACE" => {
                if let Some(sessions) = parse::<Vec<wire::Session>>(event, data) {
                    self.sessions = sessions;
                    self.own_presence_out(outcome);
                }
            }
            "USER_UPDATE" => {
                if let Some(user) = parse::<wire::User>(event, data)
                    && user.id == self.me.id
                {
                    let premium = user.premium_type.or(self.me.premium_type);
                    self.me = user;
                    self.me.premium_type = premium;
                    outcome.update(Update::Me(translate::user(&self.me)));
                }
            }
            "RELATIONSHIP_ADD" | "RELATIONSHIP_UPDATE" => self.relationship(event, data, outcome),
            "RELATIONSHIP_REMOVE" => {
                if let Some(removed) = parse::<wire::RelationshipRemove>(event, data) {
                    self.blocked.remove(&removed.id);
                    outcome.update(Update::FriendGone(snowflake::id::<UserId>(
                        removed.id.as_str(),
                    )));
                }
            }
            "MESSAGE_ACK" => {
                if let Some(ack) = parse::<wire::Ack>(event, data) {
                    let mentions = ack.mention_count.unwrap_or(0);
                    if ack.manual {
                        self.reads
                            .rewind(&ack.channel_id, ack.message_id.as_deref(), mentions);
                    } else {
                        self.reads
                            .read(&ack.channel_id, ack.message_id.as_deref(), mentions);
                    }
                    self.unread_of(&ack.channel_id, outcome);
                }
            }
            "CHANNEL_UNREAD_UPDATE" => {
                if let Some(update) = parse::<wire::UnreadUpdate>(event, data) {
                    self.last_messages(&update.channel_unread_updates, outcome);
                }
            }
            "USER_GUILD_SETTINGS_UPDATE" => {
                if let Some(settings) = parse::<wire::GuildSettings>(event, data) {
                    let guild = settings.guild_id.clone();
                    self.notifications.insert(guild.clone(), settings);
                    if let Some(guild) = guild.as_deref() {
                        self.space_out(guild, false, outcome);
                    }
                    self.refresh(guild.as_deref(), outcome);
                }
            }
            "USER_SETTINGS_PROTO_UPDATE" => {
                if let Some(update) = parse::<wire::SettingsProto>(event, data)
                    && update.settings.kind == 1
                {
                    let decoded = Preloaded::decode_base64(&update.settings.proto);
                    self.settings.absorb(decoded, update.coverage);
                    outcome.update(Update::Folders(self.settings.folders()));
                    self.own_presence_out(outcome);
                }
            }
            "VOICE_STATE_UPDATE" => self.voice_state(data, outcome),
            "VOICE_SERVER_UPDATE" => {
                if let Some(server) = parse::<wire::VoiceServer>(event, data) {
                    outcome.voice.push(Signal::Server {
                        guild: server.guild_id,
                        channel: server.channel_id,
                        token: server.token,
                        endpoint: server.endpoint,
                    });
                }
            }
            "STREAM_CREATE" | "STREAM_UPDATE" => {
                if let Some(stream) = parse::<wire::Stream>(event, data)
                    && let Ok(key) = stream.stream_key.parse()
                {
                    outcome.streams.push(StreamSignal::Created {
                        key,
                        server: stream.rtc_server_id,
                        channel: stream.rtc_channel_id,
                        viewers: stream.viewer_ids,
                        paused: stream.paused,
                    });
                }
            }
            "STREAM_SERVER_UPDATE" => {
                if let Some(server) = parse::<wire::StreamServer>(event, data)
                    && let Ok(key) = server.stream_key.parse()
                {
                    outcome.streams.push(StreamSignal::Server {
                        key,
                        token: server.token,
                        endpoint: server.endpoint,
                    });
                }
            }
            "STREAM_DELETE" => {
                if let Some(deleted) = parse::<wire::StreamDelete>(event, data)
                    && let Ok(key) = deleted.stream_key.parse()
                {
                    outcome.streams.push(StreamSignal::Deleted {
                        key,
                        reason: deleted.reason,
                        unavailable: deleted.unavailable,
                    });
                }
            }
            "CALL_CREATE" | "CALL_UPDATE" => {
                if let Some(call) = parse::<wire::Call>(event, data) {
                    let ringing = ringing(&call);
                    self.calls.insert(call.channel_id.clone(), ringing.clone());
                    outcome.update(Update::Call(ringing));
                    for state in &call.voice_states {
                        if let Some(state) = translate::voice_state(state) {
                            outcome.update(Update::Voice(state));
                        }
                    }
                }
            }
            "CALL_DELETE" => {
                if let Some(call) = parse::<wire::Call>(event, data) {
                    self.calls.remove(&call.channel_id);
                    outcome.update(Update::CallGone(snowflake::id::<ChannelId>(
                        call.channel_id.as_str(),
                    )));
                }
            }
            "GUILD_MEMBER_LIST_UPDATE" => self.member_list(data, outcome),
            "PASSIVE_UPDATE_V1" | "PASSIVE_UPDATE_V2" => self.passive(event, data, outcome),
            _ => {}
        }
    }

    fn guild_create(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(guild) = parse::<wire::Guild>("GUILD_CREATE", data) else {
            return;
        };
        if guild.unavailable {
            if let Some(known) = self.guilds.get_mut(&guild.id) {
                known.unavailable = true;
                self.space_out(&guild.id, true, outcome);
            }
            return;
        }
        let me = self.me.id.clone();
        let mine = guild
            .members
            .iter()
            .find(|member| member.user_id() == Some(me.as_str()))
            .cloned()
            .or_else(|| {
                self.guilds.get(&guild.id).map(|known| wire::Member {
                    roles: known.my_roles.clone(),
                    ..wire::Member::default()
                })
            });
        self.ingest_guild(&guild, mine.as_ref());
        self.space_out(&guild.id, true, outcome);
        let space = snowflake::id::<SpaceId>(guild.id.as_str());
        outcome.update(Update::Roles {
            space,
            roles: guild.roles.iter().map(translate::role).collect(),
        });
        outcome.update(Update::Emojis {
            space,
            emojis: guild
                .emojis
                .iter()
                .filter_map(translate::custom_emoji)
                .collect(),
        });
        outcome.update(Update::Stickers {
            space,
            stickers: guild
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
        });
        for channel in guild.channels.iter().chain(&guild.threads) {
            self.channel_out(&channel.id, true, outcome);
            self.unread_of(&channel.id, outcome);
        }
        self.members_out(&guild.id, &guild.members, outcome);
        for presence in &guild.presences {
            self.presence_out(presence, outcome);
        }
        for state in &guild.voice_states {
            if let Some(state) = voice_in(state, &guild.id) {
                outcome.update(Update::Voice(state));
            }
        }
    }

    fn guild_update(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(guild) = parse::<wire::Guild>("GUILD_UPDATE", data) else {
            return;
        };
        let Some(known) = self.guilds.get_mut(&guild.id) else {
            return;
        };
        known.properties = guild.properties();
        if !guild.roles.is_empty() {
            known.roles = guild.roles.clone();
            outcome.update(Update::Roles {
                space: snowflake::id::<SpaceId>(guild.id.as_str()),
                roles: guild.roles.iter().map(translate::role).collect(),
            });
        }
        self.space_out(&guild.id, false, outcome);
        self.refresh(Some(&guild.id), outcome);
    }

    fn guild_delete(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(removed) = parse::<wire::GuildDelete>("GUILD_DELETE", data) else {
            return;
        };
        if removed.unavailable {
            if let Some(known) = self.guilds.get_mut(&removed.id) {
                known.unavailable = true;
            }
            self.space_out(&removed.id, true, outcome);
            return;
        }
        self.guilds.remove(&removed.id);
        self.channels
            .retain(|_, tracked| tracked.guild.as_deref() != Some(removed.id.as_str()));
        self.lists.retain(|(guild, _), _| guild != &removed.id);
        self.known_members.remove(&removed.id);
        outcome.update(Update::SpaceGone(snowflake::id::<SpaceId>(
            removed.id.as_str(),
        )));
    }

    fn channel_upsert(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(wire) = parse::<wire::Channel>(event, data) else {
            return;
        };
        let known = self.channels.get(&wire.id);
        let joined = wire.member.is_some()
            || known.is_some_and(|tracked| tracked.joined)
            || (event == "THREAD_CREATE" && wire.owner_id.as_deref() == Some(self.me.id.as_str()));
        let guild = wire
            .guild_id
            .clone()
            .or_else(|| known.and_then(|tracked| tracked.guild.clone()))
            .or_else(|| {
                wire.parent_id
                    .as_deref()
                    .and_then(|parent| self.guild_of(parent))
            });
        let id = wire.id.clone();
        let last_message = wire
            .last_message_id
            .clone()
            .or_else(|| known.and_then(|tracked| tracked.wire.last_message_id.clone()));
        let mut wire = wire;
        wire.last_message_id = last_message;
        let parent_of_threads = !wire.is_thread();
        self.track(wire, guild.clone(), joined);
        self.channel_out(&id, true, outcome);
        if parent_of_threads && let Some(guild) = guild.as_deref() {
            self.refresh(Some(guild), outcome);
        }
        self.unread_of(&id, outcome);
    }

    fn thread_sync(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(sync) = parse::<wire::ThreadListSync>("THREAD_LIST_SYNC", data) else {
            return;
        };
        for thread in &sync.threads {
            let joined = sync
                .members
                .iter()
                .any(|member| member.id.as_deref() == Some(thread.id.as_str()))
                || self
                    .channels
                    .get(&thread.id)
                    .is_some_and(|tracked| tracked.joined);
            self.track(thread.clone(), Some(sync.guild_id.clone()), joined);
            self.channel_out(&thread.id, false, outcome);
            self.unread_of(&thread.id, outcome);
        }
    }

    fn thread_member(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(member) = parse::<wire::ThreadMember>("THREAD_MEMBER_UPDATE", data) else {
            return;
        };
        let Some(thread) = member.id.as_deref() else {
            return;
        };
        if let Some(tracked) = self.channels.get_mut(thread) {
            tracked.joined = true;
        }
        self.channel_out(thread, false, outcome);
    }

    fn thread_members(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(update) = parse::<wire::ThreadMembersUpdate>("THREAD_MEMBERS_UPDATE", data) else {
            return;
        };
        let me = self.me.id.clone();
        let Some(tracked) = self.channels.get_mut(&update.id) else {
            return;
        };
        if let Some(count) = update.member_count {
            tracked.wire.member_count = Some(count);
        }
        if update
            .added_members
            .iter()
            .any(|member| member.user_id.as_deref() == Some(me.as_str()))
        {
            tracked.joined = true;
        }
        if update.removed_member_ids.contains(&me) {
            tracked.joined = false;
        }
        self.channel_out(&update.id, false, outcome);
    }

    fn recipient(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(change) = parse::<wire::ChannelRecipient>(event, data) else {
            return;
        };
        let Some(tracked) = self.channels.get_mut(&change.channel_id) else {
            return;
        };
        let mut recipients = tracked.wire.recipients();
        recipients.retain(|user| user != &change.user.id);
        if event == "CHANNEL_RECIPIENT_ADD" {
            recipients.push(change.user.id.clone());
            if change.user.is_full() {
                outcome.update(Update::Users(vec![translate::user(&change.user)]));
            }
        }
        tracked.wire.recipient_ids = recipients;
        self.channel_out(&change.channel_id, false, outcome);
    }

    fn role_upsert(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(change) = parse::<wire::GuildRole>(event, data) else {
            return;
        };
        let Some(known) = self.guilds.get_mut(&change.guild_id) else {
            return;
        };
        known.roles.retain(|role| role.id != change.role.id);
        known.roles.push(change.role);
        self.roles_out(&change.guild_id, outcome);
    }

    fn role_delete(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(change) = parse::<wire::GuildRoleDelete>("GUILD_ROLE_DELETE", data) else {
            return;
        };
        let Some(known) = self.guilds.get_mut(&change.guild_id) else {
            return;
        };
        known.roles.retain(|role| role.id != change.role_id);
        known.my_roles.retain(|role| role != &change.role_id);
        self.roles_out(&change.guild_id, outcome);
    }

    fn roles_out(&mut self, guild: &str, outcome: &mut Outcome) {
        if let Some(known) = self.guilds.get(guild) {
            outcome.update(Update::Roles {
                space: snowflake::id::<SpaceId>(guild),
                roles: known.roles.iter().map(translate::role).collect(),
            });
        }
        self.refresh(Some(guild), outcome);
    }

    pub(super) fn members_out(
        &mut self,
        guild: &str,
        members: &[wire::Member],
        outcome: &mut Outcome,
    ) {
        if members.is_empty() {
            return;
        }
        let users: Vec<_> = members
            .iter()
            .filter_map(|member| member.user.as_ref())
            .filter(|user| user.is_full())
            .map(translate::user)
            .collect();
        if !users.is_empty() {
            outcome.update(Update::Users(users));
        }
        let known = self.known_members.entry(guild.to_owned()).or_default();
        for member in members {
            if let Some(user) = member.user_id() {
                known.insert(user.to_owned());
            }
        }
        let translated: Vec<_> = members
            .iter()
            .filter_map(|member| translate::member(guild, member))
            .collect();
        if !translated.is_empty() {
            outcome.update(Update::Members {
                space: snowflake::id::<SpaceId>(guild),
                members: translated,
            });
        }
        let me = self.me.id.clone();
        if let Some(mine) = members
            .iter()
            .find(|member| member.user_id() == Some(me.as_str()))
            && let Some(known) = self.guilds.get_mut(guild)
        {
            let timeout = translate::instant(mine.communication_disabled_until.as_deref());
            let changed = known.my_roles != mine.roles || known.timeout_until != timeout;
            known.my_roles = mine.roles.clone();
            known.timeout_until = timeout;
            if let Some(joined) = translate::instant(mine.joined_at.as_deref()) {
                known.joined_at = Some(joined);
            }
            if changed {
                self.refresh(Some(guild), outcome);
            }
        }
    }

    fn presence_out(&mut self, presence: &wire::Presence, outcome: &mut Outcome) {
        let Some(user) = presence.user_id() else {
            return;
        };
        if let Some(full) = presence.user.as_ref().filter(|user| user.is_full()) {
            outcome.update(Update::Users(vec![translate::user(full)]));
        }
        outcome.update(Update::Presence {
            user: snowflake::id::<UserId>(user),
            presence: presence_of(presence),
        });
    }

    fn own_presence_out(&mut self, outcome: &mut Outcome) {
        if self.me.id.is_empty() {
            return;
        }
        outcome.update(Update::Presence {
            user: snowflake::id::<UserId>(self.me.id.as_str()),
            presence: self.own_presence(self.now),
        });
    }

    fn message_create(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(wire) = parse::<wire::Message>("MESSAGE_CREATE", data) else {
            return;
        };
        let channel = wire.channel_id.clone();
        if let Some(tracked) = self.channels.get_mut(&channel)
            && tracked
                .wire
                .last_message_id
                .as_deref()
                .is_none_or(|last| snowflake::newer(&wire.id, last))
        {
            tracked.wire.last_message_id = Some(wire.id.clone());
            if tracked.wire.is_thread() {
                tracked.wire.message_count = Some(tracked.wire.message_count.unwrap_or(0) + 1);
            }
        }
        let guild = wire.guild_id.clone().or_else(|| self.guild_of(&channel));
        if let (Some(guild), Some(member), Some(author)) =
            (guild.as_deref(), wire.member.as_ref(), wire.author.as_ref())
        {
            let mut member = member.clone();
            member.user = Some(author.clone());
            self.members_out(guild, std::slice::from_ref(&member), outcome);
        }
        let arrival = self.arrival(&wire, self.channels.get(&channel));
        let Some(message) = self.message(&wire) else {
            return;
        };
        outcome.update(Update::Message(message.clone()));
        if arrival.from_me {
            self.reads.read(&channel, Some(&wire.id), 0);
        } else if arrival.counts_as_mention() {
            self.reads.mentioned(&channel);
        }
        self.unread_of(&channel, outcome);
        if arrival.notifies() {
            outcome.update(Update::Notify(Notice {
                space: guild.as_deref().map(snowflake::id::<SpaceId>),
                message: Arc::new(message),
            }));
        }
    }

    fn message_update(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(wire) = parse::<wire::Message>("MESSAGE_UPDATE", data) else {
            return;
        };
        if wire.author.is_some() {
            if let Some(message) = self.message(&wire) {
                outcome.update(Update::Message(message));
            }
            return;
        }
        if let Some(embeds) = &wire.embeds {
            outcome.update(Update::Embeds {
                channel: snowflake::id::<ChannelId>(wire.channel_id.as_str()),
                message: snowflake::id::<MessageId>(wire.id.as_str()),
                embeds: translate::embeds(embeds),
            });
        }
    }

    fn reaction(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(change) = parse::<wire::ReactionEvent>(event, data) else {
            return;
        };
        let me = change.user_id == self.me.id;
        let emoji = change.emoji.as_ref().map(translate::reaction_emoji);
        let kind = match (event, emoji) {
            ("MESSAGE_REACTION_ADD", Some(emoji)) => ReactionKind::Added { emoji, me },
            ("MESSAGE_REACTION_REMOVE", Some(emoji)) => ReactionKind::Removed { emoji, me },
            ("MESSAGE_REACTION_REMOVE_EMOJI", emoji @ Some(_)) => ReactionKind::Cleared(emoji),
            ("MESSAGE_REACTION_REMOVE_ALL", _) => ReactionKind::Cleared(None),
            _ => return,
        };
        outcome.update(Update::Reaction(ReactionChange {
            channel: snowflake::id::<ChannelId>(change.channel_id.as_str()),
            message: snowflake::id::<MessageId>(change.message_id.as_str()),
            kind,
        }));
    }

    fn reactions(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(many) = parse::<wire::ReactionMany>("MESSAGE_REACTION_ADD_MANY", data) else {
            return;
        };
        for reaction in &many.reactions {
            let emoji = translate::reaction_emoji(&reaction.emoji);
            for user in &reaction.users {
                outcome.update(Update::Reaction(ReactionChange {
                    channel: snowflake::id::<ChannelId>(many.channel_id.as_str()),
                    message: snowflake::id::<MessageId>(many.message_id.as_str()),
                    kind: ReactionKind::Added {
                        emoji: emoji.clone(),
                        me: user == &self.me.id,
                    },
                }));
            }
        }
    }

    fn typing(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(typing) = parse::<wire::Typing>("TYPING_START", data) else {
            return;
        };
        if let (Some(guild), Some(member)) = (typing.guild_id.as_deref(), typing.member.as_ref()) {
            self.members_out(guild, std::slice::from_ref(member), outcome);
        }
        outcome.update(Update::Typing {
            channel: snowflake::id::<ChannelId>(typing.channel_id.as_str()),
            user: snowflake::id::<UserId>(typing.user_id.as_str()),
            at: self.now.instant,
        });
    }

    fn relationship(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(relationship) = parse::<wire::Relationship>(event, data) else {
            return;
        };
        let user = relationship.user_id().to_owned();
        if relationship.kind == 2 {
            self.blocked.insert(user.clone());
        } else {
            self.blocked.remove(&user);
        }
        if let Some(full) = relationship.user.as_ref().filter(|user| user.is_full()) {
            outcome.update(Update::Users(vec![translate::user(full)]));
        }
        match translate::friend(&relationship) {
            Some(friend) => outcome.update(Update::Friend(friend)),
            None => outcome.update(Update::FriendGone(snowflake::id::<UserId>(&user))),
        }
    }

    fn last_messages(&mut self, updates: &[wire::ChannelUnread], outcome: &mut Outcome) {
        for update in updates {
            if let Some(tracked) = self.channels.get_mut(&update.id)
                && let Some(last) = &update.last_message_id
                && tracked
                    .wire
                    .last_message_id
                    .as_deref()
                    .is_none_or(|known| snowflake::newer(last, known))
            {
                tracked.wire.last_message_id = Some(last.clone());
                self.unread_of(&update.id, outcome);
            }
        }
    }

    fn voice_state(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(state) = parse::<wire::VoiceState>("VOICE_STATE_UPDATE", data) else {
            return;
        };
        if let (Some(guild), Some(member)) = (state.guild_id.as_deref(), state.member.as_ref()) {
            self.members_out(guild, std::slice::from_ref(member), outcome);
        }
        match translate::voice_state(&state) {
            Some(translated) => outcome.update(Update::Voice(translated)),
            None => outcome.update(Update::VoiceGone {
                user: snowflake::id::<UserId>(state.user_id.as_str()),
                space: state.guild_id.as_deref().map(snowflake::id::<SpaceId>),
            }),
        }
        if state.user_id == self.me.id
            && let Some(session) = state.session_id.clone()
        {
            outcome.voice.push(Signal::Session {
                guild: state.guild_id.clone(),
                channel: state.channel_id.clone(),
                session,
            });
        }
    }

    fn member_list(&mut self, data: &str, outcome: &mut Outcome) {
        let Some(update) = parse::<wire::MemberListUpdate>("GUILD_MEMBER_LIST_UPDATE", data) else {
            return;
        };
        let members: Vec<wire::Member> = super::roster::MemberList::members(&update.ops)
            .filter_map(|item| item.member.clone())
            .collect();
        self.members_out(&update.guild_id, &members, outcome);
        for member in &members {
            if let Some(presence) = member.presence.as_ref() {
                let mut presence = presence.clone();
                if presence.user_id().is_none() {
                    presence.user_id = member.user_id().map(str::to_owned);
                }
                self.presence_out(&presence, outcome);
            }
        }
        let key = (update.guild_id.clone(), update.id.clone());
        self.lists
            .entry(key.clone())
            .or_default()
            .apply(&update.groups, &update.ops);
        let Some(channel) = self.focus.channel else {
            return;
        };
        if self.list_id(&channel.to_string()).as_ref() == Some(&key)
            && let Some(list) = self.lists.get(&key)
        {
            outcome.update(Update::Roster {
                channel,
                roster: Arc::new(list.snapshot()),
            });
        }
    }

    fn passive(&mut self, event: &str, data: &str, outcome: &mut Outcome) {
        let Some(update) = parse::<wire::PassiveUpdate>(event, data) else {
            return;
        };
        let channels: Vec<_> = update
            .channels
            .iter()
            .chain(&update.updated_channels)
            .cloned()
            .collect();
        self.last_messages(&channels, outcome);
        for state in update
            .voice_states
            .iter()
            .chain(&update.updated_voice_states)
        {
            match voice_in(state, &update.guild_id) {
                Some(state) => outcome.update(Update::Voice(state)),
                None => outcome.update(Update::VoiceGone {
                    user: snowflake::id::<UserId>(state.user_id.as_str()),
                    space: Some(snowflake::id::<SpaceId>(update.guild_id.as_str())),
                }),
            }
        }
        for user in &update.removed_voice_states {
            outcome.update(Update::VoiceGone {
                user: snowflake::id::<UserId>(user.as_str()),
                space: Some(snowflake::id::<SpaceId>(update.guild_id.as_str())),
            });
        }
        let members: Vec<_> = update
            .members
            .iter()
            .chain(&update.updated_members)
            .cloned()
            .collect();
        self.members_out(&update.guild_id, &members, outcome);
    }
}
