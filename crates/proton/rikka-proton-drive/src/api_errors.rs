use rikka_proton_api::ResponseCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::identifier::{LinkId, RevisionId, ShareId};

const ALREADY_EXISTS: i32 = 2500;
const ALBUM_OUTSIDE_TIMELINE: i32 = 200_302;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Conflict {
    #[serde(rename = "ConflictLinkID")]
    pub(crate) link: LinkId,
    #[serde(rename = "ConflictDraftRevisionID", default)]
    pub(crate) draft_revision: Option<RevisionId>,
    #[serde(rename = "ConflictDraftClientUID", default)]
    pub(crate) draft_client: Option<String>,
}

pub(crate) fn conflict(error: &rikka_proton_api::Error) -> Option<Conflict> {
    details(error, ALREADY_EXISTS)
}

pub(crate) fn share_conflict(error: &rikka_proton_api::Error) -> Option<ShareId> {
    detail(error, ALREADY_EXISTS, "ConflictShareID")
}

pub(crate) fn album_outside_timeline(error: &rikka_proton_api::Error) -> Option<Vec<LinkId>> {
    match error.code() {
        Some(code) if code.value() == ALBUM_OUTSIDE_TIMELINE => {
            Some(detail(error, ALBUM_OUTSIDE_TIMELINE, "ChildLinkIDs").unwrap_or_default())
        }
        _ => None,
    }
}

pub(crate) fn is_missing(error: &rikka_proton_api::Error) -> bool {
    error.code() == Some(ResponseCode::NotExists)
        || error.status().is_some_and(|status| status.as_u16() == 404)
}

fn details<T: DeserializeOwned>(error: &rikka_proton_api::Error, code: i32) -> Option<T> {
    T::deserialize(refusal_details(error, code)?).ok()
}

fn detail<T: DeserializeOwned>(
    error: &rikka_proton_api::Error,
    code: i32,
    name: &str,
) -> Option<T> {
    T::deserialize(refusal_details(error, code)?.get(name)?).ok()
}

fn refusal_details(error: &rikka_proton_api::Error, code: i32) -> Option<&serde_json::Value> {
    match error {
        rikka_proton_api::Error::Api(refusal) if refusal.code.value() == code => {
            refusal.details.as_ref()
        }
        _ => None,
    }
}
