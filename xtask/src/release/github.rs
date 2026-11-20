use std::process::Command;

use anyhow::Result;

use crate::host;

pub(super) fn gh() -> Result<Command> {
    Ok(host::clean_command(&host::require("gh")?))
}
