use std::any::Any;
use std::fmt::Debug;
use std::process::Command;
use std::sync::Arc;

use rikka_agent::Function;
use rikka_prompt::{Facts, Prompt};

use crate::error::Result;

pub trait Machine: Any + Debug + Send + Sync {
    fn document(&self) -> &'static str;

    fn tool(&self, prompt: &Prompt, facts: &Facts) -> Result<Function>;

    fn isolated(&self) -> Arc<dyn Machine>;

    fn enter(&self, command: &mut Command);
}
