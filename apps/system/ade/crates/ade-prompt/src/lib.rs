mod catalog;
mod error;

pub use catalog::{AGENT, Prompts, SKILL, naming};
pub use error::{Error, Result};
pub use rikka_prompt::{Facts, Prompt};
pub use rikka_tools::Naming;
