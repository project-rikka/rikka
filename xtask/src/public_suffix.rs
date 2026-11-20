use std::path::Path;

use anyhow::Result;

use crate::{host, web};

const FILES: [(&str, &str); 2] = [
    (
        "https://publicsuffix.org/list/public_suffix_list.dat",
        "crates/foundation/rikka-public-suffix/data/public_suffix_list.dat",
    ),
    (
        "https://raw.githubusercontent.com/publicsuffix/list/main/tests/test_psl.txt",
        "crates/foundation/rikka-public-suffix/tests/test_psl.txt",
    ),
];

pub(crate) fn refresh(workspace: &Path) -> Result<()> {
    for (url, target) in FILES {
        let path = workspace.join(target);
        host::write(&path, web::bytes(url)?)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
