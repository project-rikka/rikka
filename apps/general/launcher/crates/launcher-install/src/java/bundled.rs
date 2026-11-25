use std::path::PathBuf;

use rikka_shipped::Shipped;

use super::{Java, checked};
use crate::error::{Error, Result};

const RUNTIME: Shipped = Shipped {
    variable: "RIKKA_MINECRAFT_PATH",
    installed: "lib/rikka-minecraft",
};

pub(super) fn libraries() -> Option<PathBuf> {
    RUNTIME.folder().map(|folder| folder.join("lib"))
}

pub(super) fn java(required: u32) -> Result<Java> {
    let folder = RUNTIME.folder().ok_or(Error::NoBundledJava)?;
    checked(
        folder.join("java").join("bin").join("java"),
        required,
        Some(folder.join("lib")),
    )
}
