use discord_core::{
    AUDIT_PAGE, AuditEntry, AuditQuery, Ban, MemberEdit, MemberPage, Prune, PurgeSpan, SpaceId,
    UserId,
};
use discord_translate as translate;
use discord_wire as wire;
use serde_json::{Map, Value, json};

use super::{iso, snowflakes};
use crate::client::Client;
use crate::error::Result;

const SEARCH_LIMIT: u32 = 250;
const BAN_LIMIT: u32 = 1000;
const NEWEST_FIRST: u8 = 1;

fn edit_entry(edit: &MemberEdit) -> Result<(&'static str, Value)> {
    Ok(match edit {
        MemberEdit::Nickname(nick) => ("nick", json!(nick.as_deref().unwrap_or_default())),
        MemberEdit::Roles(roles) => ("roles", snowflakes(roles)),
        MemberEdit::Timeout(until) => ("communication_disabled_until", iso(*until)?),
        MemberEdit::Mute(on) => ("mute", json!(on)),
        MemberEdit::Deaf(on) => ("deaf", json!(on)),
        MemberEdit::Channel(channel) => (
            "channel_id",
            channel.map_or(Value::Null, |id| Value::String(id.to_string())),
        ),
    })
}

impl Client {
    pub async fn search_members(
        &self,
        space: &SpaceId,
        query: &str,
    ) -> discord_core::Result<MemberPage> {
        let (space, query) = (space.to_string(), query.trim().to_owned());
        self.work(move |session| async move {
            let mut and_query = Map::new();
            if !query.is_empty() {
                and_query.insert("usernames".to_owned(), json!({ "or_query": [query] }));
            }
            let found: wire::MemberSearch = session
                .rest()
                .post(format!("/guilds/{space}/members-search"))
                .json(json!({
                    "limit": SEARCH_LIMIT,
                    "sort": NEWEST_FIRST,
                    "or_query": {},
                    "and_query": and_query,
                }))
                .parse()
                .await?;
            let members: Vec<wire::Member> = found
                .members
                .iter()
                .map(|hit| wire::Member {
                    guild_id: Some(space.clone()),
                    ..hit.member.clone()
                })
                .collect();
            let outcome = session.state().absorb_members(&space, &members);
            session.apply(outcome);
            Ok(MemberPage {
                hits: found
                    .members
                    .iter()
                    .filter_map(|hit| translate::member_hit(&space, hit))
                    .collect(),
                total: found.total_result_count,
            })
        })
        .await
    }

    pub async fn edit_member(
        &self,
        space: &SpaceId,
        user: &UserId,
        edits: Vec<MemberEdit>,
        reason: Option<String>,
    ) -> discord_core::Result<()> {
        let (space, user) = (space.to_string(), user.to_string());
        let body = edits
            .iter()
            .map(|edit| edit_entry(edit).map(|(key, value)| (key.to_owned(), value)))
            .collect::<Result<Map<String, Value>>>()?;
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/members/{user}"))
                .reason(reason.as_deref())
                .json(Value::Object(body))
                .done()
                .await
        })
        .await
    }

    pub async fn set_nickname(
        &self,
        space: &SpaceId,
        nickname: Option<String>,
    ) -> discord_core::Result<()> {
        let space = space.to_string();
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/members/@me"))
                .json(json!({ "nick": nickname.unwrap_or_default() }))
                .done()
                .await
        })
        .await
    }

    pub async fn kick(
        &self,
        space: &SpaceId,
        user: &UserId,
        reason: Option<String>,
    ) -> discord_core::Result<()> {
        let (space, user) = (space.to_string(), user.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/members/{user}"))
                .reason(reason.as_deref())
                .done()
                .await
        })
        .await
    }

    pub async fn ban(
        &self,
        space: &SpaceId,
        user: &UserId,
        purge: PurgeSpan,
        reason: Option<String>,
    ) -> discord_core::Result<()> {
        let (space, user) = (space.to_string(), user.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .put(format!("/guilds/{space}/bans/{user}"))
                .reason(reason.as_deref())
                .json(json!({ "delete_message_seconds": purge.seconds() }))
                .done()
                .await
        })
        .await
    }

    pub async fn unban(&self, space: &SpaceId, user: &UserId) -> discord_core::Result<()> {
        let (space, user) = (space.to_string(), user.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/bans/{user}"))
                .done()
                .await
        })
        .await
    }

    pub async fn bans(&self, space: &SpaceId) -> discord_core::Result<Vec<Ban>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let bans: Vec<wire::Ban> = session
                .rest()
                .get(format!("/guilds/{space}/bans"))
                .query("limit", BAN_LIMIT)
                .parse()
                .await?;
            Ok(bans.iter().map(translate::ban).collect())
        })
        .await
    }

    pub async fn prune_count(&self, space: &SpaceId, prune: &Prune) -> discord_core::Result<u32> {
        let space = space.to_string();
        let prune = prune.clone();
        self.work(move |session| async move {
            let mut request = session
                .rest()
                .get(format!("/guilds/{space}/prune"))
                .query("days", prune.days);
            if !prune.roles.is_empty() {
                let roles: Vec<String> = prune.roles.iter().map(ToString::to_string).collect();
                request = request.query("include_roles", roles.join(","));
            }
            let count: wire::PruneCount = request.parse().await?;
            Ok(count.pruned.unwrap_or(0))
        })
        .await
    }

    pub async fn prune(
        &self,
        space: &SpaceId,
        prune: &Prune,
        reason: Option<String>,
    ) -> discord_core::Result<u32> {
        let space = space.to_string();
        let prune = prune.clone();
        self.work(move |session| async move {
            let count: wire::PruneCount = session
                .rest()
                .post(format!("/guilds/{space}/prune"))
                .reason(reason.as_deref())
                .json(json!({
                    "days": prune.days,
                    "compute_prune_count": true,
                    "include_roles": snowflakes(&prune.roles),
                }))
                .parse()
                .await?;
            Ok(count.pruned.unwrap_or(0))
        })
        .await
    }

    pub async fn audit_log(
        &self,
        space: &SpaceId,
        query: &AuditQuery,
    ) -> discord_core::Result<Vec<AuditEntry>> {
        let space = space.to_string();
        let query = query.clone();
        self.work(move |session| async move {
            let mut request = session
                .rest()
                .get(format!("/guilds/{space}/audit-logs"))
                .query("limit", AUDIT_PAGE);
            if let Some(before) = query.before {
                request = request.query("before", before);
            }
            if let Some(actor) = query.actor {
                request = request.query("user_id", actor);
            }
            if let Some(action) = query.action {
                request = request.query("action_type", action.0);
            }
            let log: wire::AuditLog = request.parse().await?;
            Ok(translate::audit_log(&log))
        })
        .await
    }
}
