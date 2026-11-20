use std::path::Path;

use rikka_fs::Replace;
use rikka_llm::Media;
use sha2::{Digest, Sha256};

use crate::attachment::Attachment;
use crate::call::Call;
use crate::replay::Replay;
use crate::round::Round;
use crate::thread::Summary;
use crate::turn::{Body, Speaker, Turn, TurnText};

pub(super) const META: &str = "meta.json";
pub(crate) const JOURNAL: &str = "turns.jsonl";
pub(crate) const BLOBS: &str = "blobs";

pub(super) fn read_summary(at: &Path) -> std::result::Result<Summary, String> {
    let text = std::fs::read_to_string(at).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub(super) fn write_blob(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    Replace::new(at).private().write(bytes)
}

pub(super) fn encode_line(line: &Line) -> std::io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(line).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn stored(turn: &Turn, blobs: &Path) -> std::io::Result<StoredTurn> {
    let body = turn.body();
    let mut attached = Vec::with_capacity(body.attachments().len());
    for attachment in body.attachments() {
        attached.push(blob(attachment, blobs)?);
    }
    let mut pictured = Vec::new();
    let mut displayed = Vec::new();
    for call in body.rounds().iter().flat_map(Round::calls) {
        for picture in call.pictured() {
            pictured.push(Showing {
                call: call.id().to_owned(),
                attachment: blob(picture, blobs)?,
            });
        }
        for picture in call.displayed() {
            displayed.push(Showing {
                call: call.id().to_owned(),
                attachment: blob(picture, blobs)?,
            });
        }
    }
    Ok(StoredTurn {
        speaker: turn.speaker(),
        text: body.text().to_owned(),
        expanded: body.expanded().map(str::to_owned),
        model: body.model().to_owned(),
        replay: body.replay().cloned(),
        context: body.context().to_vec(),
        instructions: body.instructions().map(<[String]>::to_vec),
        attached,
        rounds: body.rounds().to_vec(),
        pictured,
        shown: displayed,
        at: turn.at(),
    })
}

fn blob(attachment: &Attachment, blobs: &Path) -> std::io::Result<StoredAttachment> {
    let digest = digested(attachment.data());
    let at = blobs.join(&digest);
    if !at.exists() {
        write_blob(&at, attachment.data())?;
    }
    Ok(StoredAttachment {
        name: attachment.name().to_owned(),
        media: attachment.media().mime().to_owned(),
        digest,
    })
}

pub(crate) fn restored(stored: StoredTurn, blobs: &Path) -> Body {
    let attachments = stored
        .attached
        .iter()
        .filter_map(|entry| match attached(entry, blobs) {
            Ok(attachment) => Some(attachment),
            Err(reason) => {
                tracing::warn!("failed to parse {}: {reason}", entry.name);
                None
            }
        })
        .collect();
    let mut rounds = stored.rounds;
    for showing in &stored.pictured {
        restore(&mut rounds, showing, blobs, Call::picture);
    }
    for showing in &stored.shown {
        restore(&mut rounds, showing, blobs, Call::display);
    }
    Body::new(
        TurnText {
            text: stored.text,
            expanded: stored.expanded,
            replay: stored.replay,
            context: stored.context,
            instructions: stored.instructions,
            model: stored.model,
        },
        attachments,
        rounds,
    )
}

fn restore(
    rounds: &mut [Round],
    showing: &Showing,
    blobs: &Path,
    place: impl FnOnce(&mut Call, Attachment),
) {
    let call = rounds
        .iter_mut()
        .find_map(|round| round.call(&showing.call));
    match (call, attached(&showing.attachment, blobs)) {
        (Some(call), Ok(picture)) => place(call, picture),
        (None, _) => {}
        (Some(_), Err(reason)) => {
            tracing::warn!("failed to parse {}: {reason}", showing.attachment.name);
        }
    }
}

fn attached(stored: &StoredAttachment, blobs: &Path) -> std::result::Result<Attachment, String> {
    let data = std::fs::read(blobs.join(&stored.digest)).map_err(|error| error.to_string())?;
    let media = Media::from_mime(&stored.media)
        .ok_or_else(|| format!("{} is not a type a message carries", stored.media))?;
    Attachment::new(&stored.name, media, data).map_err(|error| error.to_string())
}

pub(super) fn digested(data: &[u8]) -> String {
    rikka_types::hex::encode(Sha256::digest(data))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Line {
    Turn(StoredTurn),
    Running(StoredTurn),
    Shed,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct StoredTurn {
    pub(crate) speaker: Speaker,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expanded: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) replay: Option<Replay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) context: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instructions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) attached: Vec<StoredAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rounds: Vec<Round>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pictured: Vec<Showing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) shown: Vec<Showing>,
    pub(crate) at: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct StoredAttachment {
    name: String,
    media: String,
    digest: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Showing {
    call: String,
    #[serde(flatten)]
    attachment: StoredAttachment,
}
