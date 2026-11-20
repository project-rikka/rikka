use std::path::Path;

use rikka_icons::{FALLBACK_THEME, Icons};
use tempfile::TempDir;

fn theme(root: &Path, name: &str, index: &str, icons: &[&str]) {
    let folder = root.join(name);
    std::fs::create_dir_all(&folder).expect("create theme dir");
    std::fs::write(
        folder.join("index.theme"),
        format!("[Icon Theme]\nName={name}\n{index}"),
    )
    .expect("write index.theme");
    for icon in icons {
        touch(&folder.join(icon));
    }
}

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("path has parent")).expect("create dir");
    std::fs::write(path, []).expect("write file");
}

fn fixture_themes() -> (TempDir, Icons) {
    let root = tempfile::tempdir().expect("create temp dir");
    theme(
        root.path(),
        FALLBACK_THEME,
        "Inherits=First,Second\nDirectories=16x16/apps,48x48/apps,scalable/apps\n\n\
         [16x16/apps]\nSize=16\nType=Fixed\n\n\
         [48x48/apps]\nSize=48\nType=Fixed\n\n\
         [scalable/apps]\nSize=48\nType=Scalable\nMinSize=8\nMaxSize=256\n",
        &[
            "16x16/apps/deep.png",
            "48x48/apps/deep.png",
            "scalable/apps/browser.svg",
        ],
    );
    let plain = |extra: &str| format!("{extra}Directories=apps\n\n[apps]\nSize=32\nType=Fixed\n");
    theme(root.path(), "First", &plain("Inherits=Buried\n"), &[]);
    theme(root.path(), "Buried", &plain(""), &["apps/shared.png"]);
    theme(
        root.path(),
        "Second",
        &plain(""),
        &["apps/shared.svg", "apps/mail.png"],
    );
    touch(&root.path().join("flat.png"));
    let icons = Icons::with_root(root.path());
    (root, icons)
}

fn lookup_path(icons: &Icons, name: &str, size: u32) -> String {
    icons
        .lookup(name, size, 1)
        .unwrap_or_else(|| panic!("no icon {name} at size {size}"))
        .to_string_lossy()
        .into_owned()
}

#[test]
fn prefers_matching_size_then_closest_directory() {
    let (_root, icons) = fixture_themes();

    assert!(lookup_path(&icons, "deep", 48).contains("48x48"));
    assert!(lookup_path(&icons, "deep", 16).contains("16x16"));
    assert!(lookup_path(&icons, "deep", 20).contains("16x16"));
    assert!(lookup_path(&icons, "browser", 200).ends_with("browser.svg"));
    assert!(lookup_path(&icons, "mail", 512).ends_with("mail.png"));
}

#[test]
fn searches_inherited_themes_depth_first() {
    let (root, icons) = fixture_themes();

    let shared = icons.lookup("shared", 32, 1).expect("icon found");

    assert!(
        shared.starts_with(root.path().join("Buried")),
        "first parent's ancestors must precede second parent, got {shared:?}"
    );
}

#[test]
fn falls_back_to_unthemed_icons() {
    let (_root, icons) = fixture_themes();

    assert!(lookup_path(&icons, "flat", 48).ends_with("flat.png"));
    assert!(lookup_path(&icons, "flat.png", 48).ends_with("flat.png"));
    assert!(lookup_path(&icons, "browser.SVG", 48).ends_with("browser.svg"));
    assert!(icons.lookup("nothing-has-this", 48, 1).is_none());
}

#[test]
fn rejects_names_with_path_separators() {
    let (_root, icons) = fixture_themes();

    assert!(icons.lookup("../../etc/passwd", 48, 1).is_none());
    assert!(icons.lookup("apps/deep", 48, 1).is_none());
    assert!(icons.lookup("", 48, 1).is_none());
}

#[test]
fn absolute_path_resolves_to_itself() {
    let (root, icons) = fixture_themes();
    let loose = root.path().join("loose.png");
    touch(&loose);

    let named = icons
        .lookup(loose.to_str().expect("utf-8 path"), 48, 1)
        .expect("icon found");

    assert_eq!(&*named, loose.as_path());
    assert!(icons.lookup("/does/not/exist.png", 48, 1).is_none());
}

#[test]
fn merges_theme_across_data_dirs() {
    let home = tempfile::tempdir().expect("create data home");
    let system = tempfile::tempdir().expect("create system data dir");
    theme(
        &system.path().join("icons"),
        FALLBACK_THEME,
        "Directories=symbolic/apps\n\n[symbolic/apps]\nSize=16\nType=Scalable\nMinSize=8\nMaxSize=512\n",
        &[],
    );
    let icon = home
        .path()
        .join("icons")
        .join(FALLBACK_THEME)
        .join("symbolic/apps/app-symbolic.svg");
    touch(&icon);
    unsafe {
        std::env::set_var("XDG_DATA_HOME", home.path());
        std::env::set_var("XDG_DATA_DIRS", system.path());
    }

    let icons = Icons::new(FALLBACK_THEME);

    assert_eq!(
        icons.lookup("app-symbolic", 48, 1).as_deref(),
        Some(icon.as_path())
    );
}
