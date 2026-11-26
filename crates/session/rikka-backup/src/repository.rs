use crate::error::{Error, Result};

pub const BRANCH: &str = "main";

pub fn checked_url(url: &str) -> Result<&str> {
    let url = url.trim();
    match url.is_empty() || url.starts_with('-') {
        true => Err(Error::Invalid(format!(
            "`{url}` is not a repository address"
        ))),
        false => Ok(url),
    }
}

pub fn clone_arguments(url: &str, into: &str) -> Result<Vec<String>> {
    let url = checked_url(url)?;
    Ok([
        "clone", "--quiet", "--depth", "1", "--branch", BRANCH, "--", url, into,
    ]
    .map(str::to_owned)
    .to_vec())
}
