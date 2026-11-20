use std::sync::Arc;

use rikka_assist::{Kit, Offering};
use rikka_tool::Function;

use crate::error::Result;
use crate::prompts::{SYSTEM, facts};
use crate::tool::TOOLS;
use crate::tool_host::ToolHost;
use crate::tools;

pub static KIT: Kit = Kit::new(
    "Animate",
    "Physics-driven character animation of PMX, VRM, FBX and glTF figures: the open scene's figures, poses and motion keys, cloth and hair, wind, camera, look, timeline and renders.",
    SYSTEM,
    &TOOLS,
    facts,
);

pub fn offering(title: impl Into<String>) -> Result<Offering> {
    Ok(KIT.offering(title)?)
}

pub fn functions(tool_host: &Arc<dyn ToolHost>) -> Result<Vec<Function>> {
    tools::declare(KIT.prepared()?, tool_host)
}
