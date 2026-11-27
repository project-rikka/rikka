use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::api::time::millis;
use crate::channel::Link;
use crate::error::Result;
use crate::ids::BookmarkId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderKind {
    BookmarksBar,
    Other,
    Mobile,
    Managed,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkNode {
    pub id: BookmarkId,
    pub parent_id: Option<BookmarkId>,
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub title: String,
    pub url: Option<String>,
    #[serde(rename = "folderType")]
    pub folder_kind: Option<FolderKind>,
    #[serde(default, deserialize_with = "millis::optional::deserialize")]
    pub date_added: Option<SystemTime>,
    #[serde(default)]
    pub children: Vec<BookmarkNode>,
}

impl BookmarkNode {
    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.url.is_none()
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct NewBookmark {
    parent: BookmarkId,
    index: Option<u32>,
    title: String,
    url: Option<String>,
}

impl NewBookmark {
    pub fn page(
        parent: BookmarkId,
        title: impl Into<String>,
        url: impl Into<String>,
    ) -> NewBookmark {
        NewBookmark {
            parent,
            index: None,
            title: title.into(),
            url: Some(url.into()),
        }
    }

    pub fn folder(parent: BookmarkId, title: impl Into<String>) -> NewBookmark {
        NewBookmark {
            parent,
            index: None,
            title: title.into(),
            url: None,
        }
    }

    pub fn at(mut self, index: u32) -> NewBookmark {
        self.index = Some(index);
        self
    }

    fn request(&self) -> Value {
        let mut request = Map::new();
        request.insert("parentId".into(), json!(self.parent));
        request.insert("title".into(), json!(self.title));
        if let Some(index) = self.index {
            request.insert("index".into(), json!(index));
        }
        if let Some(url) = &self.url {
            request.insert("url".into(), json!(url));
        }
        Value::Object(request)
    }
}

#[derive(Clone, Debug)]
pub struct Bookmarks {
    link: Arc<Link>,
}

impl Bookmarks {
    pub(crate) fn new(link: Arc<Link>) -> Bookmarks {
        Bookmarks { link }
    }

    pub fn tree(&self) -> impl Future<Output = Result<Vec<BookmarkNode>>> + Send + use<> {
        self.link.call("bookmarks.getTree", Vec::new()).decoded()
    }

    pub fn children(
        &self,
        folder: &BookmarkId,
    ) -> impl Future<Output = Result<Vec<BookmarkNode>>> + Send + use<> {
        self.link
            .call("bookmarks.getChildren", vec![json!(folder)])
            .decoded()
    }

    pub fn search(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<BookmarkNode>>> + Send + use<> {
        self.link
            .call("bookmarks.search", vec![json!(query)])
            .decoded()
    }

    pub fn find_url(
        &self,
        url: &str,
    ) -> impl Future<Output = Result<Vec<BookmarkNode>>> + Send + use<> {
        self.link
            .call("bookmarks.search", vec![json!({ "url": url })])
            .decoded()
    }

    pub fn create(
        &self,
        bookmark: &NewBookmark,
    ) -> impl Future<Output = Result<BookmarkNode>> + Send + use<> {
        self.link
            .call("bookmarks.create", vec![bookmark.request()])
            .decoded()
    }

    pub fn rename(
        &self,
        id: &BookmarkId,
        title: &str,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                "bookmarks.update",
                vec![json!(id), json!({ "title": title })],
            )
            .done()
    }

    pub fn set_url(
        &self,
        id: &BookmarkId,
        url: &str,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call("bookmarks.update", vec![json!(id), json!({ "url": url })])
            .done()
    }

    pub fn move_to(
        &self,
        id: &BookmarkId,
        parent: &BookmarkId,
        index: Option<u32>,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        let mut destination = json!({ "parentId": parent });
        if let Some(index) = index {
            destination["index"] = json!(index);
        }
        self.link
            .call("bookmarks.move", vec![json!(id), destination])
            .done()
    }

    pub fn remove(&self, id: &BookmarkId) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call("bookmarks.removeTree", vec![json!(id)])
            .done()
    }
}
