use std::path::Path;

use anyhow::Result;

use crate::host;

const TABLE: &str = "crates/graphics/rikka-ltc/data/ggx.bin";

pub(crate) fn run(workspace: &Path) -> Result<()> {
    let table = rikka_ltc::fit();
    let path = workspace.join(TABLE);
    host::write(&path, table.to_bytes())?;
    println!("wrote {}", path.display());
    Ok(())
}
