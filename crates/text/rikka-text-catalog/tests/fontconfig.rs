use std::path::Path;

use rikka_text_catalog::{Catalog, CatalogConfig, LoadProblem};

use crate::support::{scratch_dir, session_font};

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("path has no parent"))
        .expect("failed to create parent dir");
    std::fs::write(
        path,
        format!("<?xml version=\"1.0\"?>\n<fontconfig>{body}</fontconfig>"),
    )
    .expect("failed to write config");
}

fn install(dir: &Path, name: &str, font: &str) {
    std::fs::create_dir_all(dir).expect("failed to create font dir");
    std::fs::copy(session_font(font), dir.join(name)).expect("failed to copy font");
}

fn families(catalog: &Catalog) -> Vec<&str> {
    let mut families: Vec<&str> = catalog.faces().map(|(_, face)| face.family()).collect();
    families.sort_unstable();
    families
}

#[test]
fn config_resolves_includes_cycles_and_broken_files() {
    let root = scratch_dir("fontconfig");
    let included = root.join("fonts/included");
    let looped = root.join("fonts/looped");
    install(&included, "Cherokee.ttf", "NotoSansCherokee.ttf");
    install(&looped, "Tifinagh.otf", "NotoSansTifinagh-Regular.otf");
    let etc = root.join("etc");
    let main = etc.join("fonts.conf");
    write(
        &main,
        &format!(
            r#"<include>{etc}/conf.d</include>
               <include>{etc}/loop.conf</include>
               <include ignore_missing="yes">/nowhere/a.conf</include>
               <include>/nowhere/b.conf</include>"#,
            etc = etc.display()
        ),
    );
    write(
        &etc.join("conf.d/10-included.conf"),
        &format!("<dir>{}</dir>", included.display()),
    );
    std::fs::write(etc.join("conf.d/20-garbage.conf"), "this is not xml")
        .expect("failed to write config");
    write(
        &etc.join("loop.conf"),
        &format!(
            "<include>{}</include><dir>{}</dir>",
            main.display(),
            looped.display()
        ),
    );

    let catalog = Catalog::load(&CatalogConfig::default().fontconfig(&main).without_cache());

    assert_eq!(
        families(&catalog),
        ["Noto Sans Cherokee", "Noto Sans Tifinagh"]
    );
    let problems = &catalog.report().problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems.iter().any(
            |problem| matches!(problem, LoadProblem::MalformedConfig(path) if path.ends_with("20-garbage.conf"))
        ),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(
            |problem| matches!(problem, LoadProblem::MissingInclude(path) if path == Path::new("/nowhere/b.conf"))
        ),
        "{problems:?}"
    );
}

#[test]
fn rejectfont_globs_hide_unless_accepted() {
    let root = scratch_dir("selectfont");
    let fonts = root.join("fonts");
    install(&fonts.join("ok"), "Cherokee.ttf", "NotoSansCherokee.ttf");
    install(&fonts.join("bad"), "OlChiki.ttf", "NotoSansOlChiki.ttf");
    install(
        &fonts.join("bad"),
        "good-1.otf",
        "NotoSansTifinagh-Regular.otf",
    );
    let main = root.join("fonts.conf");
    write(
        &main,
        &format!(
            r#"<dir>{fonts}</dir>
               <selectfont>
                 <rejectfont><glob>{fonts}/bad/*</glob></rejectfont>
                 <acceptfont><glob>{fonts}/bad/good-?.otf</glob></acceptfont>
               </selectfont>"#,
            fonts = fonts.display()
        ),
    );

    let catalog = Catalog::load(&CatalogConfig::default().fontconfig(&main).without_cache());

    assert_eq!(
        families(&catalog),
        ["Noto Sans Cherokee", "Noto Sans Tifinagh"]
    );
}
