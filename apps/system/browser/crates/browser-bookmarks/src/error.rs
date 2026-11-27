#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Chrome's bookmark tree has no Other bookmarks folder")]
    NoOtherBookmarks,
    #[error("failed to change Chrome bookmarks: {0}")]
    Remote(#[from] rikka_web::Error),
}
