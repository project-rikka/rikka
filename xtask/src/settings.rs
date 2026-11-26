use std::path::Path;

use anyhow::Result;

use crate::committed;

const CATALOG: &str = "//:settings_catalog";

pub(crate) fn regenerate(workspace: &Path) -> Result<()> {
    committed::refresh(workspace, CATALOG)
}
