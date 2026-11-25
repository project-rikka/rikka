use std::collections::HashMap;

use discord_core::{Role, RoleEdit, RoleId, SpaceId, UserId};
use discord_translate as translate;
use discord_wire as wire;
use discord_wire::snowflake;
use serde_json::{Value, json};

use super::{edits_body, image, snowflakes};
use crate::client::Client;

fn edit_entry(edit: &RoleEdit) -> (&'static str, Value) {
    match edit {
        RoleEdit::Name(name) => ("name", json!(name.trim())),
        RoleEdit::Color(color) => ("color", json!(color.unwrap_or(0))),
        RoleEdit::Hoist(on) => ("hoist", json!(on)),
        RoleEdit::Mentionable(on) => ("mentionable", json!(on)),
        RoleEdit::Permissions(permissions) => {
            ("permissions", json!(permissions.bits().to_string()))
        }
        RoleEdit::Icon(upload) => ("icon", image(upload.as_ref())),
        RoleEdit::Emoji(emoji) => ("unicode_emoji", json!(emoji)),
    }
}

impl Client {
    pub async fn create_role(&self, space: &SpaceId, name: &str) -> discord_core::Result<Role> {
        let (space, name) = (space.to_string(), name.to_owned());
        self.work(move |session| async move {
            let role: wire::Role = session
                .rest()
                .post(format!("/guilds/{space}/roles"))
                .json(json!({ "name": name, "color": 0 }))
                .parse()
                .await?;
            Ok(translate::role(&role))
        })
        .await
    }

    pub async fn edit_role(
        &self,
        space: &SpaceId,
        role: &RoleId,
        edits: Vec<RoleEdit>,
    ) -> discord_core::Result<()> {
        let (space, role) = (space.to_string(), role.to_string());
        let body = edits_body(&edits, edit_entry);
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/roles/{role}"))
                .json(body)
                .done()
                .await
        })
        .await
    }

    pub async fn delete_role(&self, space: &SpaceId, role: &RoleId) -> discord_core::Result<()> {
        let (space, role) = (space.to_string(), role.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/roles/{role}"))
                .done()
                .await
        })
        .await
    }

    pub async fn move_roles(
        &self,
        space: &SpaceId,
        positions: Vec<(RoleId, i32)>,
    ) -> discord_core::Result<()> {
        let space = space.to_string();
        let body: Vec<Value> = positions
            .iter()
            .map(|(role, position)| json!({ "id": role.to_string(), "position": position }))
            .collect();
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/roles"))
                .json(Value::Array(body))
                .done()
                .await
        })
        .await
    }

    pub async fn role_member_counts(
        &self,
        space: &SpaceId,
    ) -> discord_core::Result<HashMap<RoleId, u32>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let counts: HashMap<String, u32> = session
                .rest()
                .get(format!("/guilds/{space}/roles/member-counts"))
                .parse()
                .await?;
            Ok(counts
                .into_iter()
                .map(|(role, count)| (snowflake::id::<RoleId>(role.as_str()), count))
                .collect())
        })
        .await
    }

    pub async fn role_members(
        &self,
        space: &SpaceId,
        role: &RoleId,
    ) -> discord_core::Result<Vec<UserId>> {
        let (space, role) = (space.to_string(), role.to_string());
        self.work(move |session| async move {
            let ids: Vec<String> = session
                .rest()
                .get(format!("/guilds/{space}/roles/{role}/member-ids"))
                .parse()
                .await?;
            let users: Vec<UserId> = ids
                .iter()
                .map(|id| snowflake::id::<UserId>(id.as_str()))
                .collect();
            let missing = session
                .state()
                .missing_members(&space, ids.iter().map(String::as_str));
            for command in missing {
                session.send(command);
            }
            Ok(users)
        })
        .await
    }

    pub async fn add_role_members(
        &self,
        space: &SpaceId,
        role: &RoleId,
        users: Vec<UserId>,
    ) -> discord_core::Result<()> {
        let (space, role) = (space.to_string(), role.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/roles/{role}/members"))
                .json(json!({ "member_ids": snowflakes(&users) }))
                .done()
                .await
        })
        .await
    }

    pub async fn grant_role(
        &self,
        space: &SpaceId,
        user: &UserId,
        role: &RoleId,
        granted: bool,
    ) -> discord_core::Result<()> {
        let (space, user, role) = (space.to_string(), user.to_string(), role.to_string());
        self.work(move |session| async move {
            let path = format!("/guilds/{space}/members/{user}/roles/{role}");
            let request = match granted {
                true => session.rest().put(path),
                false => session.rest().delete(path),
            };
            request.done().await
        })
        .await
    }
}
