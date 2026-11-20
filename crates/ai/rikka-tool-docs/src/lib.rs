mod docs;
mod error;
mod naming;

pub use docs::{
    ASK, BASH, COMPUTER, DEFERRED, DESKTOP, Docs, EDIT, JOB, READING, TOOL_SEARCH,
    TOOL_SEARCH_APPS, VIEW, WRITE, facts, naming,
};
pub use error::{Error, Result};
pub use naming::Naming;
