use std::path::Path;

use ade_guide::Nearby;
use rikka_tools::PathInstructions as _;

use crate::support::project;

fn write(project: &Path, folder: &str, text: &str) {
    let at = project.join(folder);
    std::fs::create_dir_all(&at).expect("made");
    std::fs::write(at.join("RIKKA.md"), text).expect("failed to write guide");
}

#[test]
fn nearby_guides_shallowest_first() {
    let folder = project();
    write(folder.path(), "apps", "Applications.\n");
    write(folder.path(), "apps/web", "This one is TypeScript.\n");
    write(folder.path(), ".", "The project.\n");

    let nearby = Nearby::new(folder.path());
    let rendered = nearby
        .instructions_for(&[folder.path().join("apps/web/src/main.ts")])
        .expect("there are two above it");

    assert!(
        rendered.find("Applications.").unwrap() < rendered.find("This one is TypeScript.").unwrap()
    );
    assert!(!rendered.contains("The project."), "{rendered}");
}

#[test]
fn each_nearby_guide_delivered_once() {
    let folder = project();
    write(folder.path(), "apps/web", "This one is TypeScript.\n");
    let nearby = Nearby::new(folder.path());

    assert!(
        nearby
            .instructions_for(&[folder.path().join("apps/web/a.ts")])
            .is_some()
    );
    assert!(
        nearby
            .instructions_for(&[folder.path().join("apps/web/b.ts")])
            .is_none()
    );
}
