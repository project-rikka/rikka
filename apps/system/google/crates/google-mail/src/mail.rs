use std::collections::BTreeSet;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures_util::stream;
use futures_util::{StreamExt, TryStreamExt};
use google_account::{Grants, Service};
use rikka_net::{Client, RequestBuilder, Response, StatusCode, header};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::compose::Outgoing;
use crate::error::{Error, Result};
use crate::label::Label;
use crate::message::{RawThread, Summary, Thread, decoded, text};

const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const UPLOAD: &str = "https://gmail.googleapis.com/upload/gmail/v1/users/me";
const PAGE: &str = "30";
const AT_ONCE: usize = 6;
const SUMMARY_HEADERS: [&str; 2] = ["From", "Subject"];
const RELATED: &str = "rikka-mail-envelope";

#[derive(Clone, Debug)]
pub struct Mail {
    http: Client,
    grants: Arc<Grants>,
    api: String,
    upload: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Query {
    pub label: Option<String>,
    pub search: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThreadPage {
    pub threads: Vec<Summary>,
    pub next: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub address: String,
    pub history_id: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    pub threads: Vec<String>,
    pub history_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub id: String,
    pub message_id: String,
}

#[derive(Deserialize)]
struct ListedLabels {
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedThreads {
    #[serde(default)]
    threads: Vec<Listed>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct Listed {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProfile {
    email_address: String,
    #[serde(default)]
    history_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedHistory {
    #[serde(default)]
    history: Vec<Record>,
    next_page_token: Option<String>,
    #[serde(default)]
    history_id: String,
}

#[derive(Deserialize)]
struct Record {
    #[serde(default)]
    messages: Vec<Touched>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Touched {
    #[serde(default)]
    thread_id: String,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    data: String,
}

#[derive(Deserialize)]
struct Sent {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedDrafts {
    #[serde(default)]
    drafts: Vec<ListedDraft>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct ListedDraft {
    id: String,
    message: Sent,
}

impl Mail {
    #[must_use]
    pub fn google(http: Client, grants: Arc<Grants>) -> Mail {
        Mail::new(http, grants, API, UPLOAD)
    }

    #[must_use]
    pub fn new(http: Client, grants: Arc<Grants>, api: &str, upload: &str) -> Mail {
        Mail {
            http,
            grants,
            api: api.trim_end_matches('/').to_owned(),
            upload: upload.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn profile(&self) -> Result<Profile> {
        let profile: RawProfile = self.get("profile", &[]).await?;
        Ok(Profile {
            address: profile.email_address,
            history_id: profile.history_id.parse().unwrap_or(0),
        })
    }

    pub async fn labels(&self) -> Result<Vec<Label>> {
        let listed: ListedLabels = self.get("labels", &[]).await?;
        Ok(listed.labels)
    }

    pub async fn detailed_labels(&self, ids: &[String]) -> Result<Vec<Label>> {
        stream::iter(ids.to_vec())
            .map(|id| {
                let mail = self.clone();
                async move { mail.get(&format!("labels/{id}"), &[]).await }
            })
            .buffered(AT_ONCE)
            .try_collect()
            .await
    }

    pub async fn threads(&self, query: &Query, page: Option<&str>) -> Result<ThreadPage> {
        let mut params = vec![("maxResults", PAGE)];
        if let Some(label) = &query.label {
            params.push(("labelIds", label.as_str()));
        }
        let search = query.search.trim();
        if !search.is_empty() {
            params.push(("q", search));
        }
        if let Some(page) = page {
            params.push(("pageToken", page));
        }
        let listed: ListedThreads = self.get("threads", &params).await?;
        let ids: Vec<String> = listed.threads.into_iter().map(|thread| thread.id).collect();
        Ok(ThreadPage {
            threads: self.summaries(&ids).await?,
            next: listed.next_page_token,
        })
    }

    pub async fn summaries(&self, ids: &[String]) -> Result<Vec<Summary>> {
        let fetched: Vec<Result<Option<Summary>>> = stream::iter(ids.to_vec())
            .map(|id| {
                let mail = self.clone();
                async move { mail.summary(&id).await }
            })
            .buffered(AT_ONCE)
            .collect()
            .await;
        let mut summaries = Vec::with_capacity(fetched.len());
        for summary in fetched {
            summaries.extend(summary?);
        }
        Ok(summaries)
    }

    pub async fn summary(&self, id: &str) -> Result<Option<Summary>> {
        let mut params = vec![("format", "metadata")];
        params.extend(SUMMARY_HEADERS.map(|header| ("metadataHeaders", header)));
        match self
            .get::<RawThread>(&format!("threads/{id}"), &params)
            .await
        {
            Ok(thread) => Ok(thread.summary()),
            Err(Error::Missing) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn thread(&self, id: &str) -> Result<Thread> {
        let raw: RawThread = self
            .get(&format!("threads/{id}"), &[("format", "full")])
            .await?;
        let mut thread = raw.thread();
        for message in &mut thread.messages {
            for unloaded in std::mem::take(&mut message.unloaded) {
                let bytes = self
                    .attachment(&message.id, &unloaded.attachment_id)
                    .await?;
                let body = Some(text(bytes, unloaded.charset.as_deref()));
                match unloaded.html {
                    true => message.html = body,
                    false => message.text = body,
                }
            }
        }
        Ok(thread)
    }

    pub async fn attachment(&self, message_id: &str, attachment_id: &str) -> Result<Vec<u8>> {
        let content: Content = self
            .get(
                &format!("messages/{message_id}/attachments/{attachment_id}"),
                &[],
            )
            .await?;
        decoded(&content.data).ok_or(Error::Malformed("attachment data"))
    }

    pub async fn relabel(&self, thread: &str, add: &[&str], remove: &[&str]) -> Result<()> {
        let body = json!({ "addLabelIds": add, "removeLabelIds": remove });
        self.done(|token| {
            self.http
                .post(format!("{}/threads/{thread}/modify", self.api))
                .bearer_auth(token)
                .json(&body)
        })
        .await
    }

    pub async fn trash(&self, thread: &str) -> Result<()> {
        self.act(thread, "trash").await
    }

    pub async fn untrash(&self, thread: &str) -> Result<()> {
        self.act(thread, "untrash").await
    }

    pub async fn send(&self, outgoing: &Outgoing, thread: Option<&str>) -> Result<String> {
        let message = outgoing.to_rfc822()?;
        let sent: Sent = self
            .enveloped("messages/send", &threaded(json!({}), thread), &message)
            .await?;
        Ok(sent.id)
    }

    pub async fn save_draft(
        &self,
        outgoing: &Outgoing,
        thread: Option<&str>,
        replacing: Option<&str>,
    ) -> Result<Draft> {
        let raw = URL_SAFE_NO_PAD.encode(outgoing.rfc822(false)?);
        let body = json!({ "message": threaded(json!({ "raw": raw }), thread) });
        let saved: ListedDraft = self
            .call(|token| {
                let request = match replacing {
                    Some(id) => self.http.put(format!("{}/drafts/{id}", self.api)),
                    None => self.http.post(format!("{}/drafts", self.api)),
                };
                request.bearer_auth(token).json(&body)
            })
            .await?;
        Ok(Draft {
            id: saved.id,
            message_id: saved.message.id,
        })
    }

    pub async fn discard_draft(&self, id: &str) -> Result<()> {
        self.done(|token| {
            self.http
                .delete(format!("{}/drafts/{id}", self.api))
                .bearer_auth(token)
        })
        .await
    }

    pub async fn draft_for(&self, message_id: &str) -> Result<Option<Draft>> {
        let mut page: Option<String> = None;
        loop {
            let mut params = vec![("maxResults", "500")];
            if let Some(page) = &page {
                params.push(("pageToken", page.as_str()));
            }
            let listed: ListedDrafts = self.get("drafts", &params).await?;
            if let Some(draft) = listed
                .drafts
                .into_iter()
                .find(|draft| draft.message.id == message_id)
            {
                return Ok(Some(Draft {
                    id: draft.id,
                    message_id: draft.message.id,
                }));
            }
            match listed.next_page_token {
                Some(next) => page = Some(next),
                None => return Ok(None),
            }
        }
    }

    pub async fn activity(&self, since: u64) -> Result<Activity> {
        let start = since.to_string();
        let mut threads = BTreeSet::new();
        let mut page: Option<String> = None;
        loop {
            let mut params = vec![("startHistoryId", start.as_str()), ("maxResults", "500")];
            if let Some(page) = &page {
                params.push(("pageToken", page.as_str()));
            }
            let listed: ListedHistory = match self.get("history", &params).await {
                Ok(listed) => listed,
                Err(Error::Missing) => return Err(Error::HistoryExpired),
                Err(error) => return Err(error),
            };
            for record in listed.history {
                threads.extend(
                    record
                        .messages
                        .into_iter()
                        .map(|touched| touched.thread_id)
                        .filter(|thread| !thread.is_empty()),
                );
            }
            match listed.next_page_token {
                Some(next) => page = Some(next),
                None => {
                    return Ok(Activity {
                        threads: threads.into_iter().collect(),
                        history_id: listed.history_id.parse().unwrap_or(since),
                    });
                }
            }
        }
    }

    async fn act(&self, thread: &str, action: &str) -> Result<()> {
        self.done(|token| {
            self.http
                .post(format!("{}/threads/{thread}/{action}", self.api))
                .header(header::CONTENT_LENGTH, "0")
                .bearer_auth(token)
        })
        .await
    }

    async fn enveloped<T: DeserializeOwned>(
        &self,
        path: &str,
        metadata: &serde_json::Value,
        message: &[u8],
    ) -> Result<T> {
        let mut body = Vec::with_capacity(message.len() + 512);
        body.extend_from_slice(
            format!(
                "--{RELATED}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n\
                 --{RELATED}\r\nContent-Type: message/rfc822\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(message);
        body.extend_from_slice(format!("\r\n--{RELATED}--\r\n").as_bytes());
        let content_type = format!("multipart/related; boundary={RELATED}");
        self.call(|token| {
            self.http
                .post(format!("{}/{path}", self.upload))
                .query(&[("uploadType", "multipart")])
                .header(header::CONTENT_TYPE, content_type.as_str())
                .bearer_auth(token)
                .body(body.clone())
        })
        .await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        self.call(|token| {
            self.http
                .get(format!("{}/{path}", self.api))
                .query(&params)
                .bearer_auth(token)
        })
        .await
    }

    async fn call<T: DeserializeOwned>(
        &self,
        request: impl Fn(&str) -> RequestBuilder,
    ) -> Result<T> {
        let response = self.grants.send(Service::Mail, request).await?;
        Ok(accepted(response).await?.json().await?)
    }

    async fn done(&self, request: impl Fn(&str) -> RequestBuilder) -> Result<()> {
        let response = self.grants.send(Service::Mail, request).await?;
        accepted(response).await.map(|_| ())
    }
}

async fn accepted(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err(Error::Unauthorized);
    }
    Err(Error::from_answer(status, &response.text().await?))
}

fn threaded(mut message: serde_json::Value, thread: Option<&str>) -> serde_json::Value {
    if let Some(thread) = thread {
        message["threadId"] = json!(thread);
    }
    message
}
