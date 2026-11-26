use std::path::Path;

use anyhow::Result;

use crate::committed;

const BINDINGS: &str = "//sdk:bindings";

pub(crate) fn regenerate(workspace: &Path) -> Result<()> {
    committed::refresh(workspace, BINDINGS)
}
