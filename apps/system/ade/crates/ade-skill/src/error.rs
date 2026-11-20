use ade_scope::FrontMatterError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("cannot read SKILL.md: {0}")]
    Read(#[source] std::io::Error),

    #[error(transparent)]
    FrontMatter(#[from] FrontMatterError),

    #[error("missing description")]
    MissingDescription,

    #[error("skills prompt: {0}")]
    Document(#[from] rikka_prompt::Error),

    #[error("cannot declare \"{name}\": {source}")]
    Declare {
        name: String,
        #[source]
        source: rikka_tool::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
