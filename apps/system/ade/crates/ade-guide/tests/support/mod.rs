pub(crate) fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary folder")
}
