use std::time::{Duration, SystemTime};

use rikka_text_catalog::{Catalog, CatalogConfig, LoadProblem};

use crate::support::{scratch_dir, session, session_font};

#[test]
fn session_fonts_are_discovered_through_fontconfig() {
    let catalog = session();
    let families: Vec<&str> = catalog.families().collect();
    for expected in [
        "Inter",
        "Inter Variable",
        "JetBrains Mono",
        "Noto Sans",
        "Noto Serif",
        "Noto Sans Arabic",
        "Noto Sans CJK JP",
        "Noto Color Emoji",
        "Material Symbols Rounded",
        "Font Awesome 7 Free",
    ] {
        assert!(
            families
                .iter()
                .any(|family| family.eq_ignore_ascii_case(expected)),
            "{expected} is missing from {} families",
            families.len()
        );
    }
    assert!(
        catalog.faces().len() > 300,
        "{} faces",
        catalog.faces().len()
    );
}

#[test]
fn symlink_farms_are_followed_once() {
    let farm = scratch_dir("farm");
    let target = session_font("NotoSansCherokee.ttf");
    std::os::unix::fs::symlink(target, farm.join("first.ttf")).expect("failed to create symlink");
    std::fs::create_dir(farm.join("nested")).expect("failed to create nested dir");
    std::os::unix::fs::symlink(target, farm.join("nested/SECOND.TTF"))
        .expect("failed to create symlink");
    std::os::unix::fs::symlink(&farm, farm.join("nested/loop"))
        .expect("failed to create loop symlink");
    std::fs::write(farm.join("notes.txt"), "not a font").expect("failed to write file");
    let catalog = Catalog::load(
        &CatalogConfig::default()
            .font_dir(&farm)
            .cache_dir(scratch_dir("farm-index")),
    );
    assert_eq!(catalog.faces().len(), 1);
    assert_eq!(
        catalog
            .face(catalog.faces().next().expect("catalog has no faces").0)
            .family(),
        "Noto Sans Cherokee"
    );
}

#[test]
fn index_reused_until_mtime_changes() {
    let fonts = scratch_dir("indexed-fonts");
    let copy = fonts.join("Tifinagh.otf");
    std::fs::copy(session_font("NotoSansTifinagh-Regular.otf"), &copy)
        .expect("failed to copy font");
    let config = CatalogConfig::default()
        .font_dir(&fonts)
        .cache_dir(scratch_dir("index"));

    let cold = Catalog::load(&config);
    assert_eq!(
        (cold.report().scanned_files, cold.report().indexed_files),
        (1, 0)
    );
    let cached = Catalog::load(&config);
    assert_eq!(
        (cached.report().scanned_files, cached.report().indexed_files),
        (0, 1)
    );
    let (cold_id, cold_face) = cold.faces().next().expect("catalog has no faces");
    let (cached_id, cached_face) = cached.faces().next().expect("catalog has no faces");
    assert_eq!(cold_id, cached_id);
    assert_eq!(cold_face.family(), cached_face.family());
    assert_eq!(cold_face.coverage_count(), cached_face.coverage_count());
    assert!(cached_face.covers('ⵜ'));
    assert!(cached.font(cached_id).is_some());

    let file = std::fs::File::open(&copy).expect("failed to open font copy");
    file.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .expect("failed to set mtime");
    let touched = Catalog::load(&config);
    assert_eq!(
        (
            touched.report().scanned_files,
            touched.report().indexed_files
        ),
        (1, 0)
    );
}

#[test]
fn corrupt_index_is_rebuilt() {
    let fonts = scratch_dir("corrupt-fonts");
    std::fs::copy(
        session_font("NotoSansOlChiki.ttf"),
        fonts.join("OlChiki.ttf"),
    )
    .expect("failed to copy font");
    let cache = scratch_dir("corrupt-index");
    std::fs::write(cache.join("index"), b"RKFONTIX garbage").expect("failed to write index");
    let config = CatalogConfig::default().font_dir(&fonts).cache_dir(&cache);
    let rebuilt = Catalog::load(&config);
    assert_eq!(rebuilt.report().scanned_files, 1);
    assert!(
        rebuilt.report().problems.is_empty(),
        "{:?}",
        rebuilt.report().problems
    );
    assert_eq!(Catalog::load(&config).report().indexed_files, 1);
}

#[test]
fn unusable_font_files_are_reported() {
    let fonts = scratch_dir("broken-fonts");
    std::fs::write(fonts.join("broken.ttf"), b"definitely not a font")
        .expect("failed to write file");
    let catalog = Catalog::load(&CatalogConfig::default().font_dir(&fonts).without_cache());
    assert_eq!(catalog.faces().len(), 0);
    assert!(matches!(
        catalog.report().problems.as_slice(),
        [LoadProblem::UnusableFont(path)] if path.ends_with("broken.ttf")
    ));
    assert!(
        catalog
            .query(&rikka_text_catalog::Query::new(["sans-serif"]))
            .is_none()
    );
}
