use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Markup {
    Comment,
    CharacterData,
    ProcessingInstruction,
    Declaration,
    Tag,
}

impl fmt::Display for Markup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Comment => "comment",
            Self::CharacterData => "CDATA section",
            Self::ProcessingInstruction => "processing instruction",
            Self::Declaration => "declaration",
            Self::Tag => "tag",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{at}: unterminated {markup}")]
    Unterminated { markup: Markup, at: Position },
    #[error("{at}: malformed tag")]
    MalformedTag { at: Position },
    #[error("{at}: attribute `{name}` appears twice")]
    DuplicateAttribute { name: String, at: Position },
    #[error("{at}: </{found}> does not close <{open}>")]
    MismatchedEndTag {
        open: String,
        found: String,
        at: Position,
    },
    #[error("{at}: </{name}> closes nothing")]
    UnexpectedEndTag { name: String, at: Position },
    #[error("{at}: <{name}> is never closed")]
    UnclosedElement { name: String, at: Position },
    #[error("{at}: invalid character or entity reference")]
    InvalidReference { at: Position },
    #[error("{at}: namespace prefix `{prefix}` is not declared")]
    UndeclaredPrefix { prefix: String, at: Position },
    #[error("{at}: content outside the root element")]
    OutsideRoot { at: Position },
    #[error("{at}: elements nest deeper than {limit} levels")]
    TooDeep { limit: usize, at: Position },
    #[error("document has no root element")]
    NoRoot,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
