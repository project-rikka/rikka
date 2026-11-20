#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sound(#[from] rikka_audio::Error),

    #[error("failed to spawn thread `{name}`")]
    NoThread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },
}
