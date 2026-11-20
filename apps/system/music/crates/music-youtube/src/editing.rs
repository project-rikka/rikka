use music_core::{Error, Id, Result};
use serde_json::{Value, json};

use crate::json::Walk;
use crate::pages::Slot;

pub(crate) const CREATE: &str = "playlist/create";
pub(crate) const DELETE: &str = "playlist/delete";
pub(crate) const EDIT: &str = "browse/edit_playlist";
pub(crate) const UNSAVE: &str = "like/removelike";

const SUCCEEDED: &str = "STATUS_SUCCEEDED";

pub(crate) const BATCH: usize = 100;

pub(crate) fn created(name: &str, description: &str) -> Value {
    json!({ "title": name, "description": description, "privacyStatus": "PRIVATE" })
}

pub(crate) fn made(answer: &Value) -> Result<Id> {
    answer
        .text_at(&["playlistId"])
        .map(Id::new)
        .ok_or_else(|| Error::Unreadable("the new playlist came back without an id".into()))
}

pub(crate) fn named(playlist: &Id) -> Value {
    json!({ "playlistId": playlist.as_str() })
}

pub(crate) fn unsaved(playlist: &Id) -> Value {
    json!({ "target": { "playlistId": playlist.as_str() } })
}

pub(crate) fn renamed(playlist: &Id, name: &str) -> Value {
    edited(
        playlist,
        vec![json!({ "action": "ACTION_SET_PLAYLIST_NAME", "playlistName": name })],
    )
}

pub(crate) fn added(playlist: &Id, tracks: &[Id]) -> Value {
    adding(playlist, tracks, |action| {
        action["dedupeOption"] = json!("DEDUPE_OPTION_SKIP");
    })
}

pub(crate) fn appended(playlist: &Id, tracks: &[Id]) -> Value {
    adding(playlist, tracks, |_| {})
}

fn adding(playlist: &Id, tracks: &[Id], dedupe: impl Fn(&mut Value)) -> Value {
    edited(
        playlist,
        tracks
            .iter()
            .map(|track| {
                let mut action = json!({
                    "action": "ACTION_ADD_VIDEO",
                    "addedVideoId": track.as_str(),
                });
                dedupe(&mut action);
                action
            })
            .collect(),
    )
}

pub(crate) fn cleared(playlist: &Id, slots: &[Slot]) -> Value {
    edited(
        playlist,
        slots
            .iter()
            .map(|slot| {
                json!({
                    "action": "ACTION_REMOVE_VIDEO",
                    "removedVideoId": slot.video,
                    "setVideoId": slot.set,
                })
            })
            .collect(),
    )
}

pub(crate) fn removed(playlist: &Id, slots: &[Slot], track: &Id) -> Result<Value> {
    let actions: Vec<Value> = slots
        .iter()
        .filter(|slot| slot.video == track.as_str())
        .map(|slot| {
            json!({
                "action": "ACTION_REMOVE_VIDEO",
                "removedVideoId": slot.video,
                "setVideoId": slot.set,
            })
        })
        .collect();
    match actions.is_empty() {
        true => Err(Error::Missing(format!("{track} in the playlist"))),
        false => Ok(edited(playlist, actions)),
    }
}

pub(crate) fn check_status(answer: &Value) -> Result<()> {
    match answer.text_at(&["status"]) {
        None | Some(SUCCEEDED) => Ok(()),
        Some(status) => Err(Error::Refused(format!("the playlist edit ended {status}"))),
    }
}

fn edited(playlist: &Id, actions: Vec<Value>) -> Value {
    json!({ "playlistId": playlist.as_str(), "actions": actions })
}
