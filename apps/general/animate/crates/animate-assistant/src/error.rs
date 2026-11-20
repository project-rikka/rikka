#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Prompt(#[from] rikka_prompt::Error),

    #[error(transparent)]
    Assist(#[from] rikka_assist::Error),

    #[error("cannot declare tool `{name}`")]
    Declaration {
        name: String,
        #[source]
        source: rikka_tool::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
