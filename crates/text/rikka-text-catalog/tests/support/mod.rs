use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_text_catalog::{Catalog, CatalogConfig, FaceId, FaceMatch, Query};

pub(crate) fn scratch_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let base = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = base.join(format!(
        "text-catalog-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("failed to create scratch dir");
    dir
}

pub(crate) fn session() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Catalog::load(
            &CatalogConfig::from_env()
                .cache_dir(scratch_dir("session-index"))
                .locale(None),
        )
    })
}

pub(crate) fn query(families: &[&str]) -> Query {
    Query::new(families.iter().copied())
}

pub(crate) fn matched(query: &Query) -> FaceMatch {
    session()
        .query(query)
        .expect("no session font matches query")
}

pub(crate) fn family(face: FaceId) -> &'static str {
    session().face(face).family()
}

pub(crate) fn file_name(face: FaceId) -> String {
    session()
        .face(face)
        .path()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn session_font(file_name: &str) -> &'static Path {
    session()
        .faces()
        .filter_map(|(_, face)| face.path())
        .find(|path| path.file_name().is_some_and(|name| name == file_name))
        .unwrap_or_else(|| panic!("font {file_name} is not installed"))
}
