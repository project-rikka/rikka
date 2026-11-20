use std::path::Path;

use super::format::digested;

const DIGEST: usize = 8;

pub fn named(project: &Path) -> String {
    let stamp: String = digested(project.as_os_str().as_encoded_bytes())
        .chars()
        .take(DIGEST)
        .collect();
    let leaf: String = project
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(
            |letter| match letter.is_alphanumeric() || letter == '-' || letter == '_' {
                true => letter,
                false => '-',
            },
        )
        .collect();
    match leaf.is_empty() {
        true => stamp,
        false => format!("{leaf}-{stamp}"),
    }
}
