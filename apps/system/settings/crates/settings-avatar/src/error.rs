#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Image(#[from] rikka_image::Error),
    #[error("the picture has no pixels")]
    Empty,
}

pub type Result<T> = std::result::Result<T, Error>;
