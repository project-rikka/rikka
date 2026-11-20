use std::path::Path;

use ade_guide::Guides;
use ade_scope::Scope;

use crate::support::project;

fn write(project: &Path, text: &str) {
    std::fs::write(project.join("RIKKA.md"), text).expect("failed to write guide");
}

#[test]
fn renders_guides_in_named_element() {
    let folder = project();
    write(folder.path(), "Build with cargo.\n");

    let guides = Guides::find(Some(folder.path()));
    let carried = guides.get(Scope::Project).expect("it is found");

    assert_eq!(carried.text(), "Build with cargo.\n");
    assert_eq!(carried.path(), folder.path().join("RIKKA.md"));
    assert!(
        guides.render().contains(&format!(
            "<project_instructions path=\"{}\">\nBuild with cargo.\n</project_instructions>",
            carried.path().display()
        )),
        "{}",
        guides.render()
    );
}

#[test]
fn invalid_entry_is_skipped_and_reported() {
    let folder = project();
    std::fs::create_dir(folder.path().join("RIKKA.md")).expect("made");

    let guides = Guides::find(Some(folder.path()));

    assert!(guides.get(Scope::Project).is_none());
    let trouble = guides
        .diagnostics()
        .iter()
        .find(|trouble| trouble.path().starts_with(folder.path()))
        .expect("it is remarked on");
    assert!(!trouble.is_loaded());
}
