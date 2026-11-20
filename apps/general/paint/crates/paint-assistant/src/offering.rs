use std::sync::Arc;

use rikka_assist::{Kit, Offering};
use rikka_tool::Function;

use crate::error::Result;
use crate::prompts::{SYSTEM, facts};
use crate::tool::TOOLS;
use crate::tool_host::ToolHost;
use crate::tools;

pub static KIT: Kit = Kit::new(
    "Paint",
    "Edits the open pictures: view and measure, adjust tone and colour, apply effects, paint, draw paths and text, select, arrange layers, transform, undo and save.",
    SYSTEM,
    &TOOLS,
    facts,
);

pub fn offering(title: impl Into<String>) -> Result<Offering> {
    Ok(KIT.offering(title)?)
}

pub fn functions(host: &Arc<dyn ToolHost>) -> Result<Vec<Function>> {
    tools::declare(KIT.prepared()?, host)
}
