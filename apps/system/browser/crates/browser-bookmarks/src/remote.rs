use std::future::Future;

use rikka_web::{BookmarkId, Bookmarks, NewBookmark};

pub trait Remote {
    fn create(
        &self,
        parent: &BookmarkId,
        index: u32,
        title: &str,
        url: Option<&str>,
    ) -> impl Future<Output = rikka_web::Result<BookmarkId>> + Send;
    fn rename(
        &self,
        id: &BookmarkId,
        title: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send;
    fn set_url(
        &self,
        id: &BookmarkId,
        url: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send;
    fn move_to(
        &self,
        id: &BookmarkId,
        parent: &BookmarkId,
        index: u32,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send;
    fn remove(&self, id: &BookmarkId) -> impl Future<Output = rikka_web::Result<()>> + Send;
}

impl Remote for Bookmarks {
    fn create(
        &self,
        parent: &BookmarkId,
        index: u32,
        title: &str,
        url: Option<&str>,
    ) -> impl Future<Output = rikka_web::Result<BookmarkId>> + Send {
        let bookmark = match url {
            Some(url) => NewBookmark::page(parent.clone(), title, url),
            None => NewBookmark::folder(parent.clone(), title),
        };
        let created = Bookmarks::create(self, &bookmark.at(index));
        async move { Ok(created.await?.id) }
    }

    fn rename(
        &self,
        id: &BookmarkId,
        title: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        Bookmarks::rename(self, id, title)
    }

    fn set_url(
        &self,
        id: &BookmarkId,
        url: &str,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        Bookmarks::set_url(self, id, url)
    }

    fn move_to(
        &self,
        id: &BookmarkId,
        parent: &BookmarkId,
        index: u32,
    ) -> impl Future<Output = rikka_web::Result<()>> + Send {
        Bookmarks::move_to(self, id, parent, Some(index))
    }

    fn remove(&self, id: &BookmarkId) -> impl Future<Output = rikka_web::Result<()>> + Send {
        Bookmarks::remove(self, id)
    }
}
