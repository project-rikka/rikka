use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use discord_core::{Progress, Upload, UploadSource};
use futures_util::future::try_join_all;
use rikka_net::{Body, Flow, header};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Rest;
use crate::error::{Error, Result};

const SPOILER_PREFIX: &str = "SPOILER_";

#[derive(Deserialize)]
struct Slots {
    attachments: Vec<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    #[serde(deserialize_with = "slot_id")]
    id: usize,
    upload_url: String,
    upload_filename: String,
}

fn slot_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<usize, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number.as_u64().unwrap_or(0) as usize,
        Value::String(text) => text.parse().unwrap_or(0),
        _ => 0,
    })
}

pub(crate) async fn upload(
    rest: &Rest,
    channel: &str,
    uploads: &[Upload],
    progress: &Progress,
) -> Result<Vec<Value>> {
    if uploads.is_empty() {
        return Ok(Vec::new());
    }
    let names: Vec<String> = uploads.iter().map(shown_name).collect();
    let files: Vec<Value> = uploads
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(index, (upload, name))| {
            json!({
                "id": index.to_string(),
                "filename": name,
                "file_size": upload.size,
                "is_clip": false,
            })
        })
        .collect();
    let slots: Slots = rest
        .post(format!("/channels/{channel}/attachments"))
        .json(json!({ "files": files }))
        .parse()
        .await?;
    if slots.attachments.len() != uploads.len() {
        return Err(Error::Unreadable(
            "Discord offered the wrong number of upload slots".into(),
        ));
    }
    let total: u64 = uploads.iter().map(|upload| upload.size).sum();
    let sent: Arc<Vec<AtomicU64>> = Arc::new(uploads.iter().map(|_| AtomicU64::new(0)).collect());
    progress(0, total);
    let puts = slots.attachments.iter().map(|slot| {
        let upload = uploads
            .get(slot.id)
            .ok_or_else(|| Error::Unreadable("Discord offered an unknown upload slot".into()));
        let sent = Arc::clone(&sent);
        let progress = Arc::clone(progress);
        async move {
            let upload = upload?;
            let body = body(&upload.source).await?;
            let mime = upload
                .mime
                .clone()
                .or_else(|| {
                    mime_guess::from_path(&upload.name)
                        .first()
                        .map(|mime| mime.essence_str().to_owned())
                })
                .unwrap_or_else(|| "application/octet-stream".to_owned());
            let index = slot.id;
            let response = rest
                .http()
                .put(slot.upload_url.as_str())
                .header(header::CONTENT_TYPE, mime.as_str())
                .body(body)
                .on_upload_progress(move |moved: rikka_net::Progress| {
                    sent[index].store(moved.transferred, Ordering::Relaxed);
                    let done: u64 = sent.iter().map(|part| part.load(Ordering::Relaxed)).sum();
                    progress(done.min(total), total);
                    Flow::Continue
                })
                .send()
                .await
                .map_err(Error::unreachable)?;
            if !response.is_success() {
                let status = response.status().as_u16();
                let bytes = response.bytes().await.unwrap_or_default();
                return Err(super::failure(status, &bytes));
            }
            Ok::<(), Error>(())
        }
    });
    if let Err(error) = try_join_all(puts).await {
        for slot in &slots.attachments {
            let _ = rest
                .delete(format!("/attachments/{}", slot.upload_filename))
                .done()
                .await;
        }
        return Err(error);
    }
    progress(total, total);
    Ok(slots
        .attachments
        .iter()
        .map(|slot| {
            let upload = &uploads[slot.id];
            let mut attachment = json!({
                "id": slot.id.to_string(),
                "filename": names[slot.id],
                "uploaded_filename": slot.upload_filename,
            });
            if let Some(description) = upload.description.as_ref().filter(|text| !text.is_empty()) {
                attachment["description"] = json!(description);
            }
            attachment
        })
        .collect())
}

fn shown_name(upload: &Upload) -> String {
    if upload.spoiler && !upload.name.starts_with(SPOILER_PREFIX) {
        format!("{SPOILER_PREFIX}{}", upload.name)
    } else {
        upload.name.clone()
    }
}

async fn body(source: &UploadSource) -> Result<Body> {
    match source {
        UploadSource::Path(path) => Body::file(path)
            .await
            .map_err(|error| Error::Storage(error.to_string())),
        UploadSource::Bytes(bytes) => Ok(Body::bytes(rikka_net::Bytes::copy_from_slice(bytes))),
    }
}
