use std::path::PathBuf;

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Named {
    pub(crate) relative: PathBuf,
    pub(crate) folder: bool,
}

pub(crate) fn parse(typed: &str) -> Result<Named> {
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        return Err(Error::Empty);
    }
    let folder = trimmed.ends_with('/');
    let bad = || Error::BadName(trimmed.to_owned());
    if trimmed.starts_with('/') || trimmed.contains('\0') {
        return Err(bad());
    }
    let mut relative = PathBuf::new();
    for segment in trimmed.trim_end_matches('/').split('/') {
        let segment = segment.trim();
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(bad());
        }
        relative.push(segment);
    }
    Ok(Named { relative, folder })
}

pub(crate) fn parse_component(typed: &str) -> Result<String> {
    let named = parse(typed)?;
    match named.relative.components().count() == 1 && !named.folder {
        true => Ok(named.relative.to_string_lossy().into_owned()),
        false => Err(Error::BadName(typed.trim().to_owned())),
    }
}
