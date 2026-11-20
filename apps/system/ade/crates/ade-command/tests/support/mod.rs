use std::path::Path;

pub(crate) fn project() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::create_dir_all(folder.path().join(".rikka/commands")).expect("made");
    folder
}

pub(crate) fn write(project: &Path, name: &str, text: &str) {
    std::fs::write(project.join(".rikka/commands").join(name), text)
        .expect("failed to write command");
}
