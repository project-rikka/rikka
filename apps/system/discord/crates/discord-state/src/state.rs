mod dispatch;
mod ingest;
mod looks;
mod notify;
mod permissions;
mod reads;
mod roster;

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use discord_core::{
    Channel, ChannelId, Focus, Message, Presence, Ringing, Status, StreamKey, Update,
};
use serde_json::Value;

use discord_translate as translate;
use discord_wire as wire;
use discord_wire::{GuildSettings, Preloaded, Properties, commands};

use crate::moment::Moment;
use reads::{ChannelActivity, Reads};
use roster::MemberList;

pub const SUPPLEMENT_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, Default)]
pub struct Outcome {
    pub updates: Vec<Update>,
    pub commands: Vec<Value>,
    pub voice: Vec<Signal>,
    pub streams: Vec<StreamSignal>,
}

impl Outcome {
    fn update(&mut self, update: Update) {
        self.updates.push(update);
    }

    fn command(&mut self, command: Value) {
        self.commands.push(command);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    Session {
        guild: Option<String>,
        channel: Option<String>,
        session: String,
    },
    Server {
        guild: Option<String>,
        channel: Option<String>,
        token: String,
        endpoint: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamSignal {
    Created {
        key: StreamKey,
        server: Option<String>,
        channel: Option<String>,
        viewers: Vec<String>,
        paused: bool,
    },
    Server {
        key: StreamKey,
        token: String,
        endpoint: Option<String>,
    },
    Deleted {
        key: StreamKey,
        reason: Option<String>,
        unavailable: bool,
    },
}

#[derive(Clone, Debug, Default)]
struct Guild {
    properties: Properties,
    roles: Vec<wire::Role>,
    my_roles: Vec<String>,
    timeout_until: Option<i64>,
    joined_at: Option<i64>,
    unavailable: bool,
    shown: Option<discord_core::Space>,
}

#[derive(Clone, Debug)]
struct Tracked {
    wire: wire::Channel,
    guild: Option<String>,
    joined: bool,
    shown: Option<Channel>,
}

struct Pending {
    ready: wire::Ready,
    buffered: Vec<(String, String)>,
    since: i64,
}

pub struct State {
    now: Moment,
    me: wire::User,
    settings: Preloaded,
    sessions: Vec<wire::Session>,
    guilds: HashMap<String, Guild>,
    channels: HashMap<String, Tracked>,
    notifications: HashMap<Option<String>, GuildSettings>,
    reads: Reads,
    blocked: HashSet<String>,
    calls: HashMap<String, Ringing>,
    pending: Option<Pending>,
    focus: Focus,
    ranges: Vec<[u32; 2]>,
    lists: HashMap<(String, String), MemberList>,
    known_members: HashMap<String, HashSet<String>>,
    ticked: i64,
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("me", &self.me.id)
            .field("guilds", &self.guilds.len())
            .field("channels", &self.channels.len())
            .finish_non_exhaustive()
    }
}

impl State {
    #[must_use]
    pub fn new(now: Moment) -> State {
        State {
            now,
            me: wire::User::default(),
            settings: Preloaded::default(),
            sessions: Vec::new(),
            guilds: HashMap::new(),
            channels: HashMap::new(),
            notifications: HashMap::new(),
            reads: Reads::default(),
            blocked: HashSet::new(),
            calls: HashMap::new(),
            pending: None,
            focus: Focus::default(),
            ranges: Vec::new(),
            lists: HashMap::new(),
            known_members: HashMap::new(),
            ticked: now.unix_ms,
        }
    }

    #[must_use]
    pub fn me(&self) -> &str {
        &self.me.id
    }

    #[must_use]
    pub fn premium(&self) -> u8 {
        self.me.premium_type.unwrap_or(0)
    }

    #[must_use]
    pub fn locale(&self) -> Option<&str> {
        self.settings.locale()
    }

    #[must_use]
    pub fn guild_of(&self, channel: &str) -> Option<String> {
        self.channels
            .get(channel)
            .and_then(|tracked| tracked.guild.clone())
    }

    #[must_use]
    pub fn is_private(&self, channel: &str) -> bool {
        self.channels
            .get(channel)
            .is_some_and(|tracked| tracked.wire.is_private())
    }

    #[must_use]
    pub fn kind_of(&self, channel: &str) -> Option<u8> {
        self.channels.get(channel).map(|tracked| tracked.wire.kind)
    }

    #[must_use]
    pub fn call_running(&self, channel: &str) -> bool {
        self.calls.contains_key(channel)
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.settings
            .status()
            .unwrap_or_else(|| self.session_presence().status)
    }

    #[must_use]
    pub fn custom_activity(&self, now: Moment) -> Option<Value> {
        let custom = self.settings.custom_status(now.unix_ms)?;
        let mut activity = serde_json::json!({
            "name": "Custom Status",
            "type": 4,
            "state": custom.text,
        });
        if !custom.emoji_name.is_empty() {
            activity["emoji"] = serde_json::json!({
                "name": custom.emoji_name,
                "id": (custom.emoji_id != 0).then(|| custom.emoji_id.to_string()),
            });
        }
        Some(activity)
    }

    #[must_use]
    pub fn own_presence(&self, now: Moment) -> Presence {
        let mut presence = self.session_presence();
        presence.status = self.status();
        if let Some(custom) = self.settings.custom_status(now.unix_ms) {
            let text = Some(custom.text.clone()).filter(|text| !text.is_empty());
            let emoji = (custom.emoji_id == 0 && !custom.emoji_name.is_empty())
                .then(|| custom.emoji_name.clone());
            presence.mood = match (emoji, text) {
                (Some(emoji), Some(text)) => Some(format!("{emoji} {text}")),
                (emoji, text) => text.or(emoji),
            };
        }
        presence
    }

    fn session_presence(&self) -> Presence {
        let session = self
            .sessions
            .iter()
            .find(|session| session.session_id == "all")
            .or_else(|| self.sessions.first());
        translate::presence(
            session.and_then(|session| session.status.as_deref()),
            session.map_or(&[], |session| session.activities.as_slice()),
            None,
        )
    }

    pub fn dispatch(&mut self, event: &str, data: &str, now: Moment) -> Outcome {
        self.now = now;
        let mut outcome = Outcome::default();
        if event == "READY" {
            self.ready(data);
            return outcome;
        }
        if let Some(pending) = self.pending.as_mut() {
            if event == "READY_SUPPLEMENTAL" {
                self.supplemental(data, &mut outcome);
            } else {
                pending.buffered.push((event.to_owned(), data.to_owned()));
            }
            return outcome;
        }
        self.route(event, data, &mut outcome);
        outcome
    }

    pub fn focus(&mut self, focus: Focus) -> Outcome {
        let mut outcome = Outcome::default();
        let changed = focus.channel != self.focus.channel;
        self.focus = focus;
        if changed {
            self.ranges = roster::ranges(0..1);
        }
        self.subscribe(&mut outcome);
        outcome
    }

    pub fn view_roster(&mut self, channel: ChannelId, rows: Range<u32>) -> Outcome {
        let mut outcome = Outcome::default();
        if self.focus.channel != Some(channel) {
            return outcome;
        }
        let ranges = roster::ranges(rows);
        if ranges != self.ranges {
            self.ranges = ranges;
            self.subscribe(&mut outcome);
        }
        outcome
    }

    pub fn mark_read(&mut self, channel: &str, message: &str) -> Outcome {
        let mut outcome = Outcome::default();
        self.reads.read(channel, Some(message), 0);
        self.unread_of(channel, &mut outcome);
        outcome
    }

    pub fn unread_in(&self, guild: &str) -> Vec<(String, String)> {
        self.channels
            .iter()
            .filter(|(_, tracked)| tracked.guild.as_deref() == Some(guild))
            .filter_map(|(id, tracked)| {
                let last = tracked.wire.last_message_id.as_deref()?;
                let unread = self.reads.unread(&self.activity(id, tracked));
                (unread.unread || unread.mentions > 0).then(|| (id.clone(), last.to_owned()))
            })
            .collect()
    }

    pub fn missing_members<'a>(
        &mut self,
        guild: &str,
        users: impl Iterator<Item = &'a str>,
    ) -> Vec<Value> {
        let known = self.known_members.entry(guild.to_owned()).or_default();
        let mut missing: Vec<String> = users
            .filter(|user| known.insert((*user).to_owned()))
            .map(str::to_owned)
            .collect();
        missing.sort();
        missing.dedup();
        if missing.is_empty() {
            return Vec::new();
        }
        commands::request_members(guild, &missing)
    }

    pub fn adopt(
        &mut self,
        wires: &[wire::Channel],
        joined: &[String],
        now: Moment,
    ) -> Vec<Channel> {
        self.now = now;
        let mut shown = Vec::new();
        for wire in wires {
            let joined = wire.member.is_some()
                || joined.contains(&wire.id)
                || self
                    .channels
                    .get(&wire.id)
                    .is_some_and(|tracked| tracked.joined);
            let guild = wire.guild_id.clone().or_else(|| {
                wire.parent_id
                    .as_deref()
                    .and_then(|parent| self.guild_of(parent))
            });
            self.track(wire.clone(), guild, joined);
            if let Some(channel) = self.channel_now(&wire.id) {
                shown.push(channel);
            }
        }
        shown
    }

    pub fn message(&self, wire: &wire::Message) -> Option<Message> {
        let guild = wire
            .guild_id
            .clone()
            .or_else(|| self.guild_of(&wire.channel_id));
        translate::message(wire, &self.reader(guild.as_deref()))
    }

    pub fn tick(&mut self, now: Moment) -> Outcome {
        self.now = now;
        let mut outcome = Outcome::default();
        let now = now.unix_ms;
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| now - pending.since >= SUPPLEMENT_WAIT.as_millis() as i64)
        {
            self.supplemental("{}", &mut outcome);
        }
        let since = std::mem::replace(&mut self.ticked, now);
        if !self.deadline_passed(since, now) {
            return outcome;
        }
        let spaces: Vec<String> = self.guilds.keys().cloned().collect();
        for guild in spaces {
            self.space_out(&guild, false, &mut outcome);
        }
        self.refresh(None, &mut outcome);
        outcome
    }

    fn deadline_passed(&self, since: i64, now: i64) -> bool {
        let passed = |end: Option<i64>| end.is_some_and(|end| end > since && end <= now);
        let mute_end = |config: Option<&wire::MuteConfig>| {
            translate::instant(config.and_then(|config| config.end_time.as_deref()))
        };
        self.guilds
            .values()
            .any(|guild| passed(guild.timeout_until))
            || self.notifications.values().any(|settings| {
                passed(mute_end(settings.mute_config.as_ref()))
                    || settings
                        .channel_overrides
                        .iter()
                        .any(|entry| passed(mute_end(entry.mute_config.as_ref())))
            })
    }

    fn activity<'a>(&'a self, id: &'a str, tracked: &'a Tracked) -> ChannelActivity<'a> {
        let tracked_since = match &tracked.guild {
            Some(_) if tracked.wire.is_thread() && !tracked.joined => None,
            Some(guild) => self.guilds.get(guild).and_then(|guild| guild.joined_at),
            None => None,
        };
        ChannelActivity {
            channel: id,
            last_message: tracked.wire.last_message_id.as_deref(),
            tracked_since,
        }
    }

    fn unread_of(&self, channel: &str, outcome: &mut Outcome) {
        if let Some(tracked) = self.channels.get(channel) {
            outcome.update(Update::Unread(
                self.reads.unread(&self.activity(channel, tracked)),
            ));
        }
    }

    fn subscribe(&self, outcome: &mut Outcome) {
        let (Some(space), Some(channel)) = (&self.focus.space, &self.focus.channel) else {
            return;
        };
        let (space, channel_key) = (space.to_string(), channel.to_string());
        let Some(tracked) = self.channels.get(&channel_key) else {
            return;
        };
        if tracked.guild.as_deref() != Some(space.as_str()) {
            return;
        }
        let listed = self.listed_channel(&channel_key);
        outcome.command(commands::subscribe(&space, &listed, &self.ranges));
        if let Some(list) = self.list_for(&channel_key) {
            outcome.update(Update::Roster {
                channel: *channel,
                roster: std::sync::Arc::new(list.snapshot()),
            });
        }
    }

    fn listed_channel(&self, channel: &str) -> String {
        match self.channels.get(channel) {
            Some(tracked) if tracked.wire.is_thread() => tracked
                .wire
                .parent_id
                .clone()
                .unwrap_or_else(|| channel.to_owned()),
            _ => channel.to_owned(),
        }
    }

    fn list_id(&self, channel: &str) -> Option<(String, String)> {
        let listed = self.listed_channel(channel);
        let tracked = self.channels.get(&listed)?;
        let guild_id = tracked.guild.clone()?;
        let guild = self.guilds.get(&guild_id)?;
        let everyone = guild
            .roles
            .iter()
            .find(|role| role.id == guild_id)
            .map_or(0, |role| role.permissions);
        Some((
            guild_id,
            roster::list_id(&tracked.wire.permission_overwrites, everyone),
        ))
    }

    fn list_for(&self, channel: &str) -> Option<&MemberList> {
        self.lists.get(&self.list_id(channel)?)
    }
}
