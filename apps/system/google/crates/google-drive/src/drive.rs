use std::path::Path;
use std::sync::Arc;

use google_account::{Grants, Service};
use rikka_net::{
    Bytes, CancelToken, Client, Flow, Method, Overwrite, Progress, RequestBuilder, Response,
    StatusCode, header,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::file::{FIELDS, FOLDER, File, Kind};
use crate::scope::Scope;

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";
const PAGE: &str = "200";
const LISTED_AT_MOST: usize = 5000;
const UPLOAD_LENGTH: &str = "X-Upload-Content-Length";

#[derive(Clone, Debug)]
pub struct Drive {
    http: Client,
    grants: Arc<Grants>,
    api: String,
    upload: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub files: Vec<File>,
    pub next: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Storage {
    pub limit: Option<u64>,
    pub usage: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub file_id: String,
    pub file: Option<File>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes {
    pub changes: Vec<Change>,
    pub cursor: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Export {
    pub mime: &'static str,
    pub extension: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Listed {
    #[serde(default)]
    files: Vec<File>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedDrives {
    #[serde(default)]
    drives: Vec<SharedDrive>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct SharedDrive {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct About {
    storage_quota: Quota,
}

#[derive(Deserialize)]
struct Quota {
    limit: Option<String>,
    #[serde(default)]
    usage: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartCursor {
    start_page_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedChanges {
    #[serde(default)]
    changes: Vec<ListedChange>,
    next_page_token: Option<String>,
    new_start_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedChange {
    #[serde(default)]
    file_id: String,
    #[serde(default)]
    removed: bool,
    file: Option<File>,
}

impl Export {
    pub const PDF: Export = Export {
        mime: "application/pdf",
        extension: "pdf",
    };

    #[must_use]
    pub fn editable(kind: Kind) -> Option<Export> {
        let (mime, extension) = match kind {
            Kind::Document => (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "docx",
            ),
            Kind::Spreadsheet => (
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xlsx",
            ),
            Kind::Presentation => (
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "pptx",
            ),
            Kind::Drawing => ("image/png", "png"),
            _ => return None,
        };
        Some(Export { mime, extension })
    }
}

impl Drive {
    #[must_use]
    pub fn google(http: Client, grants: Arc<Grants>) -> Drive {
        Drive::new(http, grants, API, UPLOAD)
    }

    #[must_use]
    pub fn new(http: Client, grants: Arc<Grants>, api: &str, upload: &str) -> Drive {
        Drive {
            http,
            grants,
            api: api.trim_end_matches('/').to_owned(),
            upload: upload.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn storage(&self) -> Result<Storage> {
        let about: About = self
            .call(|token| {
                self.http
                    .get(format!("{}/about", self.api))
                    .query(&[("fields", "storageQuota(limit,usage)")])
                    .bearer_auth(token)
            })
            .await?;
        Ok(Storage {
            limit: about
                .storage_quota
                .limit
                .and_then(|limit| limit.parse().ok()),
            usage: about.storage_quota.usage.parse().unwrap_or(0),
        })
    }

    pub async fn list(&self, scope: &Scope, page: Option<&str>) -> Result<Page> {
        if *scope == Scope::SharedDrives {
            return self.shared_drives(page).await;
        }
        let query = scope.query();
        let fields = format!("nextPageToken,files({FIELDS})");
        let mut params = vec![
            ("q", query.filter.as_str()),
            ("fields", fields.as_str()),
            ("pageSize", PAGE),
            ("spaces", "drive"),
            ("supportsAllDrives", "true"),
            ("includeItemsFromAllDrives", "true"),
        ];
        if let Some(order) = query.order {
            params.push(("orderBy", order));
        }
        if let Some(page) = page {
            params.push(("pageToken", page));
        }
        let listed: Listed = self
            .call(|token| {
                self.http
                    .get(format!("{}/files", self.api))
                    .query(&params)
                    .bearer_auth(token)
            })
            .await?;
        Ok(Page {
            files: listed.files,
            next: listed.next_page_token,
        })
    }

    pub async fn list_all(&self, scope: &Scope) -> Result<Vec<File>> {
        let mut files = Vec::new();
        let mut page = None;
        loop {
            let listed = self.list(scope, page.as_deref()).await?;
            files.extend(listed.files);
            page = listed.next;
            if page.is_none() || files.len() >= LISTED_AT_MOST {
                return Ok(files);
            }
        }
    }

    pub async fn create_folder(&self, parent: &str, name: &str) -> Result<File> {
        let body = json!({ "name": name, "mimeType": FOLDER, "parents": [parent] });
        self.call(|token| {
            self.http
                .post(format!("{}/files", self.api))
                .query(&[("fields", FIELDS), ("supportsAllDrives", "true")])
                .bearer_auth(token)
                .json(&body)
        })
        .await
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<File> {
        self.patch(id, &json!({ "name": name }), &[]).await
    }

    pub async fn set_starred(&self, id: &str, starred: bool) -> Result<File> {
        self.patch(id, &json!({ "starred": starred }), &[]).await
    }

    pub async fn set_trashed(&self, id: &str, trashed: bool) -> Result<File> {
        self.patch(id, &json!({ "trashed": trashed }), &[]).await
    }

    pub async fn relocate(&self, id: &str, from: &[String], to: &str) -> Result<File> {
        let from = from.join(",");
        self.patch(
            id,
            &json!({}),
            &[("addParents", to), ("removeParents", from.as_str())],
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.done(|token| {
            self.http
                .delete(self.file(id))
                .query(&[("supportsAllDrives", "true")])
                .bearer_auth(token)
        })
        .await
    }

    pub async fn empty_trash(&self) -> Result<()> {
        self.done(|token| {
            self.http
                .delete(format!("{}/files/trash", self.api))
                .bearer_auth(token)
        })
        .await
    }

    pub async fn download(
        &self,
        file: &File,
        destination: &Path,
        cancel: CancelToken,
        progress: impl Fn(Progress) + Clone + Send + 'static,
    ) -> Result<()> {
        if file.is_native() || file.is_folder() {
            return Err(Error::NotDownloadable);
        }
        let url = format!(
            "{}?alt=media&supportsAllDrives=true",
            self.file(file.target_id())
        );
        self.fetch(&url, destination, cancel, progress).await
    }

    pub async fn export(
        &self,
        file: &File,
        export: Export,
        destination: &Path,
        cancel: CancelToken,
    ) -> Result<()> {
        if !file.is_native() {
            return Err(Error::NotDownloadable);
        }
        let mut url = rikka_net::Url::parse(&format!("{}/export", self.file(file.target_id())))
            .map_err(|_| Error::Malformed("export address"))?;
        url.query_pairs_mut().append_pair("mimeType", export.mime);
        self.fetch(url.as_str(), destination, cancel, |_| {}).await
    }

    pub async fn upload(
        &self,
        source: &Path,
        parent: &str,
        name: &str,
        cancel: CancelToken,
        progress: impl Fn(Progress) + Clone + Send + 'static,
    ) -> Result<File> {
        let length = std::fs::metadata(source)
            .map_err(|source_error| Error::File {
                path: source.to_owned(),
                source: source_error,
            })?
            .len();
        let metadata = json!({ "name": name, "parents": [parent] });
        let opened = self
            .grants
            .send(Service::Drive, |token| {
                self.http
                    .post(format!("{}/files", self.upload))
                    .query(&[
                        ("uploadType", "resumable"),
                        ("supportsAllDrives", "true"),
                        ("fields", FIELDS),
                    ])
                    .header(UPLOAD_LENGTH, length.to_string())
                    .bearer_auth(token)
                    .json(&metadata)
            })
            .await?;
        let opened = accepted(opened).await?;
        let session = opened
            .headers()
            .get(header::LOCATION)
            .and_then(|location| location.to_str().ok())
            .ok_or(Error::Malformed("no upload session"))?
            .to_owned();
        let token = self.grants.token(Service::Drive).await?;
        let stored = self
            .http
            .upload(session, source)
            .method(Method::PUT)
            .bearer_auth(token.expose())
            .cancel_token(cancel)
            .on_progress(move |moved| {
                progress(moved);
                Flow::Continue
            })
            .send()
            .await?;
        Ok(accepted(stored).await?.json().await?)
    }

    pub async fn thumbnail(&self, file: &File, pixels: u32) -> Result<Bytes> {
        let link = file.thumbnail(pixels).ok_or(Error::Missing)?;
        let response = self
            .grants
            .send(Service::Drive, |token| {
                self.http.get(link.as_str()).bearer_auth(token)
            })
            .await?;
        Ok(accepted(response).await?.bytes().await?)
    }

    pub async fn start_cursor(&self) -> Result<String> {
        let start: StartCursor = self
            .call(|token| {
                self.http
                    .get(format!("{}/changes/startPageToken", self.api))
                    .query(&[("supportsAllDrives", "true")])
                    .bearer_auth(token)
            })
            .await?;
        Ok(start.start_page_token)
    }

    pub async fn changes(&self, cursor: &str) -> Result<Changes> {
        let fields =
            format!("nextPageToken,newStartPageToken,changes(fileId,removed,file({FIELDS}))");
        let mut changes = Vec::new();
        let mut page = cursor.to_owned();
        loop {
            let listed: ListedChanges = self
                .call(|token| {
                    self.http
                        .get(format!("{}/changes", self.api))
                        .query(&[
                            ("pageToken", page.as_str()),
                            ("fields", fields.as_str()),
                            ("pageSize", PAGE),
                            ("supportsAllDrives", "true"),
                            ("includeItemsFromAllDrives", "true"),
                            ("includeRemoved", "true"),
                        ])
                        .bearer_auth(token)
                })
                .await?;
            changes.extend(listed.changes.into_iter().map(|change| Change {
                file: change.file.filter(|_| !change.removed),
                file_id: change.file_id,
            }));
            match (listed.next_page_token, listed.new_start_page_token) {
                (Some(next), _) => page = next,
                (None, Some(cursor)) => return Ok(Changes { changes, cursor }),
                (None, None) => return Err(Error::Malformed("changes without a cursor")),
            }
        }
    }

    async fn shared_drives(&self, page: Option<&str>) -> Result<Page> {
        let mut params = vec![
            ("pageSize", "100"),
            ("fields", "nextPageToken,drives(id,name)"),
        ];
        if let Some(page) = page {
            params.push(("pageToken", page));
        }
        let listed: ListedDrives = self
            .call(|token| {
                self.http
                    .get(format!("{}/drives", self.api))
                    .query(&params)
                    .bearer_auth(token)
            })
            .await?;
        Ok(Page {
            files: listed
                .drives
                .into_iter()
                .map(|drive| File {
                    drive_id: Some(drive.id.clone()),
                    id: drive.id,
                    name: drive.name,
                    mime_type: FOLDER.to_owned(),
                    ..File::default()
                })
                .collect(),
            next: listed.next_page_token,
        })
    }

    async fn patch(&self, id: &str, body: &Value, extra: &[(&str, &str)]) -> Result<File> {
        let mut params = vec![("fields", FIELDS), ("supportsAllDrives", "true")];
        params.extend_from_slice(extra);
        self.call(|token| {
            self.http
                .patch(self.file(id))
                .query(&params)
                .bearer_auth(token)
                .json(body)
        })
        .await
    }

    async fn fetch(
        &self,
        url: &str,
        destination: &Path,
        cancel: CancelToken,
        progress: impl Fn(Progress) + Clone + Send + 'static,
    ) -> Result<()> {
        let mut token = self.grants.token(Service::Drive).await?;
        for renewed in [false, true] {
            let report = progress.clone();
            let fetched = self
                .http
                .download(url, destination)
                .overwrite(Overwrite::Replace)
                .bearer_auth(token.expose())
                .cancel_token(cancel.clone())
                .on_progress(move |moved| {
                    report(moved);
                    Flow::Continue
                })
                .start()
                .await;
            match fetched {
                Ok(_) => return Ok(()),
                Err(error) if error.status() == Some(StatusCode::UNAUTHORIZED) && !renewed => {
                    token = self.grants.renew(Service::Drive, &token).await?;
                }
                Err(error) => return Err(refused(error)),
            }
        }
        Err(Error::Unauthorized)
    }

    async fn call<T: DeserializeOwned>(
        &self,
        request: impl Fn(&str) -> RequestBuilder,
    ) -> Result<T> {
        let response = self.grants.send(Service::Drive, request).await?;
        Ok(accepted(response).await?.json().await?)
    }

    async fn done(&self, request: impl Fn(&str) -> RequestBuilder) -> Result<()> {
        let response = self.grants.send(Service::Drive, request).await?;
        accepted(response).await.map(|_| ())
    }

    fn file(&self, id: &str) -> String {
        format!("{}/files/{id}", self.api)
    }
}

async fn accepted(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(Error::from_answer(status, &response.text().await?))
}

fn refused(error: rikka_net::Error) -> Error {
    match error.status() {
        Some(StatusCode::NOT_FOUND) => Error::Missing,
        Some(StatusCode::UNAUTHORIZED) => Error::Unauthorized,
        Some(StatusCode::TOO_MANY_REQUESTS) => Error::RateLimited,
        _ => Error::Net(error),
    }
}
