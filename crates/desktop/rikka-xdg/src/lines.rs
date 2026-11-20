use std::io;
use std::path::Path;

pub(crate) fn read(path: &Path) -> io::Result<Option<Vec<String>>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.lines().map(str::to_owned).collect())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn write(path: &Path, lines: &[String]) -> io::Result<()> {
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    rikka_fs::Replace::new(path).write(text)
}

pub(crate) fn is_read_only(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ReadOnlyFilesystem | io::ErrorKind::PermissionDenied
    )
}
