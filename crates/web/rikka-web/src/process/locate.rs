use std::path::PathBuf;

use rikka_shipped::Shipped;

use crate::error::{Error, Result};

const GECKO: Shipped = Shipped {
    variable: "RIKKA_GECKO_PATH",
    installed: "lib/rikka-gecko",
};

const PROGRAM: &str = "rikka-gecko";

pub(crate) fn program() -> Result<PathBuf> {
    GECKO.find(PROGRAM).ok_or(Error::NotInstalled)
}
