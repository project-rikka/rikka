use std::path::Path;

use music_core::{Account, Error, Result, keep_secret, kept_secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const KIND: &str = "session";
const LABEL: &str = "Qobuz sign-in";

#[derive(Serialize, Deserialize)]
struct SavedSession {
    token: String,
}

pub(crate) async fn load_token(folder: &Path) -> Result<Option<String>> {
    Ok(kept_secret::<SavedSession>(folder, KIND)
        .await?
        .map(|saved| saved.token))
}

pub(crate) async fn save_token(folder: &Path, token: &str) -> Result<()> {
    keep_secret(
        folder,
        KIND,
        LABEL,
        &SavedSession {
            token: token.to_owned(),
        },
    )
    .await
}

pub(crate) struct Member {
    pub(crate) id: u64,
    pub(crate) account: Account,
}

pub(crate) fn member(login: &Value) -> Result<Member> {
    let user = login
        .get("user")
        .ok_or_else(|| Error::Unreadable("Qobuz returned no signed-in user".into()))?;
    let id = user
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::Unreadable("Qobuz user has no id".into()))?;
    if !has_streaming_subscription(user) {
        return Err(Error::NotEntitled(
            "this Qobuz account has no streaming subscription".into(),
        ));
    }
    let field = |key: &str| {
        user.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let name = field("display_name")
        .or_else(|| field("login"))
        .or_else(|| field("email"))
        .unwrap_or_else(|| id.to_string());
    Ok(Member {
        id,
        account: Account {
            id: id.to_string(),
            name,
        },
    })
}

fn has_streaming_subscription(user: &Value) -> bool {
    let Some(parameters) = user
        .get("credential")
        .and_then(|credential| credential.get("parameters"))
        .and_then(Value::as_object)
    else {
        return false;
    };
    ["lossy_streaming", "lossless_streaming", "hires_streaming"]
        .iter()
        .any(|flag| parameters.get(*flag).and_then(Value::as_bool) == Some(true))
}
