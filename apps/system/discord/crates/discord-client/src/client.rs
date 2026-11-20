use std::ops::Range;
use std::time::Duration;

use std::sync::Arc;

use base64::Engine as _;
use discord_core::{
    Channel, ChannelId, DISCORD_EPOCH_MS, Draft, Emoji, Focus, ForumPage, ForumPost, Limits,
    Message, MessageId, Page, Profile, Progress, SpaceId, Status, StreamKey, TagId, Update, UserId,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};

use discord_state::Moment;
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::{Preloaded, channel_kind, commands, snowflake};

use crate::error::{Error, Result};
use crate::rest;
use crate::session::Session;
use crate::stream::{Broadcast, Streams, Watch};
use crate::voice::{Call, Voice};

const PAGE_SIZE: u32 = 50;
const SEARCH_PAGE: u32 = 25;
const MAX_THREADS: u32 = 100;
const INDEX_TRIES: u32 = 5;
const ACKS_PER_REQUEST: usize = 100;
const MIB: u64 = 1024 * 1024;
const MAX_UPLOADS: usize = 10;
const SILENT: u64 = 1 << 12;
const ARCHIVE_AFTER_MINUTES: u32 = 1440;
const PUBLIC_THREAD: u8 = 11;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const REACTION_KEY: &AsciiSet = &NON_ALPHANUMERIC.remove(b':').remove(b'_').remove(b'-');

#[derive(Clone, Debug)]
pub struct Client {
    session: Arc<Session>,
}

fn limits_for(premium: u8) -> Limits {
    let (upload_bytes, message_chars) = match premium {
        2 => (500 * MIB, 4000),
        1 | 3 => (50 * MIB, 2000),
        _ => (10 * MIB, 2000),
    };
    Limits {
        upload_bytes,
        message_chars,
        uploads_per_message: MAX_UPLOADS,
    }
}

fn reaction_key(emoji: &Emoji) -> String {
    let key = match &emoji.id {
        Some(id) => format!("{}:{}", emoji.name, id),
        None => emoji.name.clone(),
    };
    utf8_percent_encode(&key, REACTION_KEY).to_string()
}

fn days_since_epoch() -> i64 {
    (Moment::now().unix_ms - DISCORD_EPOCH_MS) / DAY_MS + 1
}

fn message_of(session: &Session, wire: &wire::Message) -> Result<Message> {
    session
        .state()
        .message(wire)
        .ok_or_else(|| Error::Unreadable("Discord answered with an incomplete message".into()))
}

fn check(session: &Session, draft: &Draft) -> Result<()> {
    let limits = limits_for(session.state().premium());
    if draft.text.chars().count() > limits.message_chars {
        return Err(Error::TooLarge(format!(
            "a message of {} characters",
            draft.text.chars().count()
        )));
    }
    if draft.uploads.len() > limits.uploads_per_message {
        return Err(Error::TooLarge(format!(
            "{} files in one message",
            draft.uploads.len()
        )));
    }
    if let Some(upload) = draft
        .uploads
        .iter()
        .find(|upload| upload.size > limits.upload_bytes)
    {
        return Err(Error::TooLarge(upload.name.clone()));
    }
    Ok(())
}

async fn search_threads(
    session: &Session,
    channel: &str,
    query: &[(&str, String)],
) -> Result<wire::ThreadSearch> {
    for _ in 0..INDEX_TRIES {
        let mut request = session
            .rest()
            .get(format!("/channels/{channel}/threads/search"));
        for (name, value) in query {
            request = request.query(name, value);
        }
        let (status, bytes) = request.respond().await?;
        let results: wire::ThreadSearch =
            serde_json::from_slice(&bytes).map_err(Error::unreadable)?;
        if status != 202 {
            return Ok(results);
        }
        let wait = results.retry_after.unwrap_or(1.0).max(1.0);
        tokio::time::sleep(Duration::from_secs_f64(wait)).await;
    }
    Err(Error::Unreachable(
        "Discord is still indexing these threads".into(),
    ))
}

fn joined_threads(session: &Session, results: &wire::ThreadSearch) -> Vec<String> {
    let me = session.state().me().to_owned();
    results
        .members
        .iter()
        .filter(|member| member.user_id.as_deref().is_none_or(|user| user == me))
        .filter_map(|member| member.id.clone())
        .collect()
}

fn adopt_one(session: &Session, wire: &wire::Channel) -> Result<Channel> {
    let channel = session
        .state()
        .adopt(
            std::slice::from_ref(wire),
            std::slice::from_ref(&wire.id),
            Moment::now(),
        )
        .into_iter()
        .next()
        .ok_or_else(|| Error::Unreadable("Discord answered with an unknown channel".into()))?;
    session.emit(Update::Channel(channel.clone()));
    Ok(channel)
}

fn message_body(draft: &Draft, attachments: Vec<Value>) -> Value {
    let mut body = json!({
        "content": draft.text,
        "nonce": draft.nonce,
        "enforce_nonce": true,
        "tts": false,
        "flags": if draft.silent { SILENT } else { 0 },
        "mobile_network_type": "unknown",
    });
    if !attachments.is_empty() {
        body["attachments"] = Value::Array(attachments);
    }
    if !draft.stickers.is_empty() {
        body["sticker_ids"] = json!(
            draft
                .stickers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
    }
    body
}

impl Client {
    pub(crate) fn new(session: Arc<Session>) -> Client {
        Client { session }
    }

    #[must_use]
    pub fn limits(&self) -> Limits {
        limits_for(self.session.premium())
    }

    #[must_use]
    pub fn mint(&self) -> MessageId {
        self.session.mint()
    }

    pub fn focus(&self, focus: Focus) {
        self.session.in_background(move |state| state.focus(focus));
    }

    pub fn view_roster(&self, channel: &ChannelId, rows: Range<u32>) {
        let channel = *channel;
        self.session
            .in_background(move |state| state.view_roster(channel, rows));
    }

    pub async fn messages(
        &self,
        channel: &ChannelId,
        page: Page,
    ) -> discord_core::Result<Vec<Message>> {
        let channel = channel.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let mut request = session
                    .rest()
                    .get(format!("/channels/{channel}/messages"))
                    .query("limit", PAGE_SIZE);
                request = match &page {
                    Page::Latest => request,
                    Page::Before(id) => request.query("before", id),
                    Page::After(id) => request.query("after", id),
                    Page::Around(id) => request.query("around", id),
                };
                let wires: Vec<wire::Message> = request.parse().await?;
                let (messages, member_requests) = {
                    let mut state = session.state();
                    let messages: Vec<Message> = wires
                        .iter()
                        .filter_map(|wire| state.message(wire))
                        .collect();
                    let member_requests = match state.guild_of(&channel) {
                        Some(guild) => state.missing_members(
                            &guild,
                            wires
                                .iter()
                                .filter(|wire| wire.webhook_id.is_none())
                                .filter_map(|wire| {
                                    wire.author.as_ref().map(|author| author.id.as_str())
                                }),
                        ),
                        None => Vec::new(),
                    };
                    (messages, member_requests)
                };
                for command in member_requests {
                    session.send(command);
                }
                Ok(messages)
            })
            .await?)
    }

    pub async fn send(
        &self,
        channel: &ChannelId,
        draft: Draft,
        progress: Progress,
    ) -> discord_core::Result<Message> {
        let channel = channel.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                check(&session, &draft)?;
                let attachments =
                    rest::upload(session.rest(), &channel, &draft.uploads, &progress).await?;
                let mut body = message_body(&draft, attachments);
                if let Some(reply) = &draft.reply {
                    let guild = session.state().guild_of(&channel);
                    body["message_reference"] = json!({
                        "message_id": reply.message,
                        "channel_id": channel,
                        "guild_id": guild,
                        "fail_if_not_exists": false,
                    });
                    body["allowed_mentions"] = json!({
                        "parse": ["users", "roles", "everyone"],
                        "replied_user": reply.mention,
                    });
                }
                let wire: wire::Message = session
                    .rest()
                    .post(format!("/channels/{channel}/messages"))
                    .context(&json!({ "location": "chat_input" }))
                    .json(body)
                    .parse()
                    .await?;
                message_of(&session, &wire)
            })
            .await?)
    }

    pub async fn edit(
        &self,
        channel: &ChannelId,
        message: &MessageId,
        text: &str,
    ) -> discord_core::Result<Message> {
        let path = format!("/channels/{channel}/messages/{message}");
        let text = text.to_owned();
        Ok(self
            .session
            .work(move |session| async move {
                let wire: wire::Message = session
                    .rest()
                    .patch(path)
                    .json(json!({ "content": text }))
                    .parse()
                    .await?;
                message_of(&session, &wire)
            })
            .await?)
    }

    pub async fn delete(
        &self,
        channel: &ChannelId,
        message: &MessageId,
    ) -> discord_core::Result<()> {
        let path = format!("/channels/{channel}/messages/{message}");
        Ok(self
            .session
            .work(move |session| async move { session.rest().delete(path).done().await })
            .await?)
    }

    pub async fn react(
        &self,
        channel: &ChannelId,
        message: &MessageId,
        emoji: &Emoji,
        on: bool,
    ) -> discord_core::Result<()> {
        let base = format!(
            "/channels/{channel}/messages/{message}/reactions/{}",
            reaction_key(emoji)
        );
        Ok(self
            .session
            .work(move |session| async move {
                let rest = session.rest();
                let request = if on {
                    rest.put(format!("{base}/%40me"))
                        .query("location", "Message")
                        .query("type", 0)
                } else {
                    rest.delete(format!("{base}/0/%40me"))
                        .query("location", "Message")
                        .query("burst", false)
                };
                request.done().await
            })
            .await?)
    }

    pub async fn pin(
        &self,
        channel: &ChannelId,
        message: &MessageId,
        on: bool,
    ) -> discord_core::Result<()> {
        let path = format!("/channels/{channel}/pins/{message}");
        Ok(self
            .session
            .work(move |session| async move {
                if on {
                    session.rest().put(path).done().await
                } else {
                    session.rest().delete(path).done().await
                }
            })
            .await?)
    }

    pub async fn pinned(&self, channel: &ChannelId) -> discord_core::Result<Vec<Message>> {
        let path = format!("/channels/{channel}/pins");
        Ok(self
            .session
            .work(move |session| async move {
                let wires: Vec<wire::Message> = session.rest().get(path).parse().await?;
                let state = session.state();
                Ok(wires
                    .iter()
                    .filter_map(|wire| state.message(wire))
                    .collect())
            })
            .await?)
    }

    pub async fn typing(&self, channel: &ChannelId) -> discord_core::Result<()> {
        let path = format!("/channels/{channel}/typing");
        Ok(self
            .session
            .work(move |session| async move { session.rest().post(path).done().await })
            .await?)
    }

    pub async fn mark_read(
        &self,
        channel: &ChannelId,
        message: &MessageId,
    ) -> discord_core::Result<()> {
        let channel = channel.to_string();
        let message = message.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let (flags, outcome) = {
                    let mut state = session.state();
                    let flags = match (state.kind_of(&channel), state.guild_of(&channel)) {
                        (
                            Some(
                                channel_kind::NEWS_THREAD
                                | channel_kind::PUBLIC_THREAD
                                | channel_kind::PRIVATE_THREAD,
                            ),
                            _,
                        ) => 2,
                        (_, Some(_)) => 1,
                        _ => 0,
                    };
                    (flags, state.mark_read(&channel, &message))
                };
                session.apply(outcome);
                session
                    .rest()
                    .post(format!("/channels/{channel}/messages/{message}/ack"))
                    .json(
                        json!({ "token": null, "last_viewed": days_since_epoch(), "flags": flags }),
                    )
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn mark_space_read(&self, space: &SpaceId) -> discord_core::Result<()> {
        let space = space.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let unread = session.state().unread_in(&space);
                for (channel, message) in &unread {
                    let outcome = session.state().mark_read(channel, message);
                    session.apply(outcome);
                }
                for batch in unread.chunks(ACKS_PER_REQUEST) {
                    let states: Vec<Value> = batch
                        .iter()
                        .map(|(channel, message)| {
                            json!({ "channel_id": channel, "message_id": message, "read_state_type": 0 })
                        })
                        .collect();
                    session
                        .rest()
                        .post("/read-states/ack-bulk")
                        .json(json!({ "read_states": states }))
                        .done()
                        .await?;
                }
                Ok(())
            })
            .await?)
    }

    pub async fn open_direct(&self, user: &UserId) -> discord_core::Result<ChannelId> {
        let user = user.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let wire: wire::Channel = session
                    .rest()
                    .post("/users/@me/channels")
                    .context(&json!({}))
                    .json(json!({ "recipients": [user] }))
                    .parse()
                    .await?;
                adopt_one(&session, &wire)?;
                Ok(snowflake::id::<ChannelId>(wire.id.as_str()))
            })
            .await?)
    }

    pub async fn close_direct(&self, channel: &ChannelId) -> discord_core::Result<()> {
        let channel = channel.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let group = session.state().kind_of(&channel) == Some(channel_kind::GROUP);
                let mut request = session.rest().delete(format!("/channels/{channel}"));
                if group {
                    request = request.query("silent", false);
                }
                request.done().await
            })
            .await?)
    }

    pub async fn threads(
        &self,
        channel: &ChannelId,
        archived: bool,
    ) -> discord_core::Result<Vec<Channel>> {
        let channel = channel.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let mut threads = Vec::new();
                let mut offset = 0;
                loop {
                    let mut query = vec![
                        ("archived", archived.to_string()),
                        ("sort_by", "last_message_time".to_owned()),
                        ("sort_order", "desc".to_owned()),
                        ("limit", SEARCH_PAGE.to_string()),
                    ];
                    if offset > 0 {
                        query.push(("offset", offset.to_string()));
                    }
                    let results = search_threads(&session, &channel, &query).await?;
                    let joined = joined_threads(&session, &results);
                    let adopted = session
                        .state()
                        .adopt(&results.threads, &joined, Moment::now());
                    offset += results.threads.len() as u32;
                    threads.extend(adopted);
                    if !results.has_more || results.threads.is_empty() || offset >= MAX_THREADS {
                        return Ok(threads);
                    }
                }
            })
            .await?)
    }

    pub async fn create_thread(
        &self,
        channel: &ChannelId,
        from: Option<&MessageId>,
        name: &str,
    ) -> discord_core::Result<Channel> {
        let path = match from {
            Some(message) => format!("/channels/{channel}/messages/{message}/threads"),
            None => format!("/channels/{channel}/threads"),
        };
        let mut body = json!({ "name": name, "auto_archive_duration": ARCHIVE_AFTER_MINUTES });
        if from.is_none() {
            body["type"] = json!(PUBLIC_THREAD);
        }
        Ok(self
            .session
            .work(move |session| async move {
                let wire: wire::Channel = session.rest().post(path).json(body).parse().await?;
                adopt_one(&session, &wire)
            })
            .await?)
    }

    pub async fn join_thread(&self, thread: &ChannelId, join: bool) -> discord_core::Result<()> {
        let path = format!("/channels/{thread}/thread-members/%40me");
        Ok(self
            .session
            .work(move |session| async move {
                if join {
                    session.rest().put(path).done().await
                } else {
                    session.rest().delete(path).done().await
                }
            })
            .await?)
    }

    pub async fn forum_posts(
        &self,
        forum: &ChannelId,
        offset: u32,
    ) -> discord_core::Result<ForumPage> {
        let forum = forum.to_string();
        Ok(self
            .session
            .work(move |session| async move {
                let mut query = vec![
                    ("sort_by", "last_message_time".to_owned()),
                    ("sort_order", "desc".to_owned()),
                    ("limit", SEARCH_PAGE.to_string()),
                    ("tag_setting", "match_some".to_owned()),
                ];
                if offset > 0 {
                    query.push(("offset", offset.to_string()));
                }
                let results = search_threads(&session, &forum, &query).await?;
                let mut starters: std::collections::HashMap<String, wire::Message> = results
                    .first_messages
                    .iter()
                    .map(|message| (message.channel_id.clone(), message.clone()))
                    .collect();
                let missing: Vec<&str> = results
                    .threads
                    .iter()
                    .map(|thread| thread.id.as_str())
                    .filter(|id| !starters.contains_key(*id))
                    .collect();
                if !missing.is_empty() {
                    let posts: wire::PostData = session
                        .rest()
                        .post(format!("/channels/{forum}/post-data"))
                        .json(json!({ "thread_ids": missing }))
                        .parse()
                        .await?;
                    for (thread, post) in posts.threads {
                        if let Some(message) = post.first_message {
                            starters.insert(thread, message);
                        }
                    }
                }
                let joined = joined_threads(&session, &results);
                let state_threads = session
                    .state()
                    .adopt(&results.threads, &joined, Moment::now());
                let posts = state_threads
                    .into_iter()
                    .map(|thread| {
                        let starter = starters
                            .get(&thread.id.to_string())
                            .and_then(|wire| session.state().message(wire));
                        let replies = thread.thread.as_ref().map_or(0, |meta| meta.messages);
                        ForumPost {
                            thread,
                            starter,
                            replies,
                        }
                    })
                    .collect();
                Ok(ForumPage {
                    posts,
                    more: results.has_more,
                })
            })
            .await?)
    }

    pub async fn create_post(
        &self,
        forum: &ChannelId,
        title: &str,
        tags: &[TagId],
        draft: Draft,
        progress: Progress,
    ) -> discord_core::Result<Channel> {
        let forum = forum.to_string();
        let title = title.to_owned();
        let tags: Vec<String> = tags.iter().map(|tag| tag.to_string()).collect();
        Ok(self
            .session
            .work(move |session| async move {
                check(&session, &draft)?;
                let attachments =
                    rest::upload(session.rest(), &forum, &draft.uploads, &progress).await?;
                let mut message = json!({ "content": draft.text });
                if !attachments.is_empty() {
                    message["attachments"] = Value::Array(attachments);
                }
                if !draft.stickers.is_empty() {
                    message["sticker_ids"] = json!(
                        draft
                            .stickers
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    );
                }
                let created: wire::Created = session
                    .rest()
                    .post(format!("/channels/{forum}/threads"))
                    .query("use_nested_fields", true)
                    .json(json!({
                        "name": title,
                        "auto_archive_duration": ARCHIVE_AFTER_MINUTES,
                        "applied_tags": tags,
                        "message": message,
                    }))
                    .parse()
                    .await?;
                adopt_one(&session, &created.thread)
            })
            .await?)
    }

    pub async fn profile(
        &self,
        user: &UserId,
        space: Option<&SpaceId>,
    ) -> discord_core::Result<Profile> {
        let user = user.to_string();
        let space = space.map(ToString::to_string);
        Ok(self
            .session
            .work(move |session| async move {
                let other = session.state().me() != user;
                let mut request = session
                    .rest()
                    .get(format!("/users/{user}/profile"))
                    .query("type", "popout")
                    .query("with_mutual_guilds", other)
                    .query("with_mutual_friends", other)
                    .query("with_mutual_friends_count", false);
                if let Some(space) = &space {
                    request = request.query("guild_id", space);
                }
                let wire: wire::Profile = request.parse().await?;
                Ok(translate::profile(&wire, space.as_deref()))
            })
            .await?)
    }

    pub async fn set_status(&self, status: Status) -> discord_core::Result<()> {
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .patch("/users/@me/settings-proto/1")
                    .json(json!({ "settings": Preloaded::status_patch(status) }))
                    .done()
                    .await?;
                let activities: Vec<Value> = session
                    .state()
                    .custom_activity(Moment::now())
                    .into_iter()
                    .collect();
                session.send(commands::presence(status, activities));
                Ok(())
            })
            .await?)
    }

    pub async fn request_friend_by_name(&self, handle: &str) -> discord_core::Result<()> {
        let (username, discriminator) = match handle.trim().rsplit_once('#') {
            Some((name, tag))
                if tag.len() == 4 && tag.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                (name.to_owned(), tag.parse::<u32>().ok())
            }
            _ => (handle.trim().trim_start_matches('@').to_owned(), None),
        };
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .post("/users/@me/relationships")
                    .context(&json!({ "location": "Add Friend" }))
                    .json(json!({ "username": username, "discriminator": discriminator }))
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn accept_friend(&self, user: &UserId) -> discord_core::Result<()> {
        self.relate(user, "Friends").await
    }

    pub async fn request_friend(&self, user: &UserId) -> discord_core::Result<()> {
        self.relate(user, "User Profile").await
    }

    async fn relate(&self, user: &UserId, location: &'static str) -> discord_core::Result<()> {
        let path = format!("/users/@me/relationships/{user}");
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .put(path)
                    .context(&json!({ "location": location }))
                    .json(json!({}))
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn unfriend(&self, user: &UserId) -> discord_core::Result<()> {
        let path = format!("/users/@me/relationships/{user}");
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .delete(path)
                    .context(&json!({ "location": "Friends" }))
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn note(&self, user: &UserId) -> discord_core::Result<String> {
        let path = format!("/users/@me/notes/{user}");
        Ok(self
            .session
            .work(move |session| async move {
                match session.rest().get(path).parse::<wire::Note>().await {
                    Ok(note) => Ok(note.note.unwrap_or_default()),
                    Err(Error::Missing(_)) => Ok(String::new()),
                    Err(error) => Err(error),
                }
            })
            .await?)
    }

    pub async fn set_note(&self, user: &UserId, note: &str) -> discord_core::Result<()> {
        let path = format!("/users/@me/notes/{user}");
        let note = note.to_owned();
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .put(path)
                    .json(json!({ "note": note }))
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn join_voice(
        &self,
        channel: &ChannelId,
        muted: bool,
        deafened: bool,
    ) -> discord_core::Result<Call> {
        let channel = channel.to_string();
        Ok(self
            .session
            .work(move |session| Voice::join(session, channel, muted, deafened))
            .await?)
    }

    pub async fn go_live(&self, channel: &ChannelId) -> discord_core::Result<Broadcast> {
        let channel = *channel;
        Ok(self
            .session
            .work(move |session| Streams::go_live(session, channel))
            .await?)
    }

    pub async fn watch(&self, key: StreamKey) -> discord_core::Result<Watch> {
        Ok(self
            .session
            .work(move |session| Streams::watch(session, key))
            .await?)
    }

    pub async fn upload_preview(&self, key: StreamKey, jpeg: Vec<u8>) -> discord_core::Result<()> {
        let path = format!("/streams/{key}/preview");
        let thumbnail = format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(jpeg)
        );
        Ok(self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .post(path)
                    .json(json!({ "thumbnail": thumbnail }))
                    .done()
                    .await
            })
            .await?)
    }

    pub async fn decline_call(&self, channel: &ChannelId) -> discord_core::Result<()> {
        let path = format!("/channels/{channel}/call/stop-ringing");
        Ok(
            self.session
                .work(move |session| async move {
                    session.rest().post(path).json(json!({})).done().await
                })
                .await?,
        )
    }

    pub async fn sign_out(&self) -> discord_core::Result<()> {
        let result = self
            .session
            .work(move |session| async move {
                session
                    .rest()
                    .post("/auth/logout")
                    .json(json!({ "provider": null, "voip_provider": null }))
                    .done()
                    .await
            })
            .await;
        self.session.close();
        Ok(result?)
    }
}
