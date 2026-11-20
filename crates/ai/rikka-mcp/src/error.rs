#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to read {path}: {reason}")]
    Unreadable { path: String, reason: String },

    #[error("failed to write {path}: {reason}")]
    Unwritable { path: String, reason: String },

    #[error("invalid MCP server catalog {document}: {reason}")]
    Malformed { document: String, reason: String },

    #[error("unterminated quote in command line")]
    Unclosed,

    #[error("{} not set", unset_named(.names))]
    Unset { names: Vec<String> },

    #[error("unknown MCP server scope \"{0}\"")]
    Placeless(String),

    #[error("no MCP server named \"{0}\"")]
    Unknown(String),

    #[error("\"{0}\" does not use HTTP and has no sign-in")]
    Local(String),

    #[error("sign-in to \"{server}\" failed: {reason}")]
    SignIn { server: String, reason: String },

    #[error("sign-out from \"{server}\" failed: {reason}")]
    SignOut { server: String, reason: String },

    #[error("MCP server \"{0}\" is not connected")]
    Disconnected(String),

    #[error("no MCP prompt /{0}")]
    Unprompted(String),

    #[error("/{0} takes no arguments")]
    Unasked(String),

    #[error("/{command} needs {argument}")]
    Unfilled { command: String, argument: String },

    #[error("MCP server \"{0}\" offers no resources")]
    Resourceless(String),

    #[error("@{0} names no MCP resource")]
    Unmentioned(String),

    #[error("MCP server \"{server}\" failed: {reason}")]
    Unanswered { server: String, reason: String },
}

fn unset_named(names: &[String]) -> String {
    let named: Vec<String> = names.iter().map(|name| format!("${{{name}}}")).collect();
    match named.as_slice() {
        [one] => format!("{one} is"),
        _ => format!("{} are", named.join(", ")),
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
