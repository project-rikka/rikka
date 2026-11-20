use std::path::Path;

pub(crate) fn same_file(source: &Path, destination: &Path) -> bool {
    let settled = |path: &Path| {
        path.canonicalize().unwrap_or_else(|_| {
            let parent = path.parent().unwrap_or(Path::new("."));
            let folder = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            match path.file_name() {
                Some(name) => folder.join(name),
                None => folder,
            }
        })
    };
    settled(source) == settled(destination)
}
