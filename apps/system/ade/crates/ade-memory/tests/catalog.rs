use std::path::Path;

use ade_memory::Memories;

fn project() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::create_dir_all(folder.path().join(".rikka/memory")).expect("made");
    folder
}

fn note(root: &Path, name: &str, text: &str) {
    std::fs::write(root.join(".rikka/memory").join(name), text).expect("failed to write note");
}

fn described(title: &str, description: &str, about: Option<&str>) -> String {
    let about = about.map_or_else(String::new, |about| format!("about: {about}\n"));
    format!("---\ntitle: {title}\ndescription: {description}\n{about}---\n\nBody.\n")
}

#[test]
fn notes_grouped_by_about() {
    let folder = project();
    std::fs::create_dir_all(folder.path().join("apps/system/ade")).expect("made");
    std::fs::create_dir_all(folder.path().join("system")).expect("made");

    note(
        folder.path(),
        "how-a-build-runs.md",
        &described("How a build runs", "Through nix develop.", None),
    );
    note(
        folder.path(),
        "the-ade.md",
        &described(
            "The ADE",
            "A project beside a conversation.",
            Some("apps/system/ade"),
        ),
    );
    note(
        folder.path(),
        "the-compositor.md",
        &described(
            "The compositor",
            "DRM/KMS, or nested under winit.",
            Some("system"),
        ),
    );

    let memories = Memories::find(Some(folder.path()));
    let rendered = memories.render();

    assert_eq!(memories.all().len(), 3);
    assert!(
        memories.diagnostics().is_empty(),
        "{:?}",
        memories.diagnostics()
    );
    assert!(
        rendered.contains("- [How a build runs](how-a-build-runs.md) — Through nix develop."),
        "{rendered}"
    );
    assert!(rendered.contains("## apps/system/ade"), "{rendered}");
    assert!(rendered.contains("## system"), "{rendered}");
    assert!(
        rendered.find("How a build runs") < rendered.find("## apps/system/ade"),
        "what is about no one place comes first: {rendered}"
    );
    assert!(
        rendered.find("## apps/system/ade") < rendered.find("## system"),
        "the headings are sorted: {rendered}"
    );
}

#[test]
fn index_written_only_on_change() {
    let folder = project();
    note(
        folder.path(),
        "the-toolchain.md",
        &described("The toolchain", "Rust 2024, one workspace.", None),
    );

    let memories = Memories::find(Some(folder.path()));
    assert!(memories.write_index().expect("it writes"));

    let index = std::fs::read_to_string(
        Memories::index(Some(folder.path())).expect("a project has an index"),
    )
    .expect("it is there");
    assert!(index.contains("# Project memory"), "{index}");
    assert!(
        index.contains("[The toolchain](the-toolchain.md)"),
        "{index}"
    );
    assert!(!memories.write_index().expect("it reads"));
}

#[test]
fn index_and_non_markdown_are_not_notes() {
    let folder = project();
    note(
        folder.path(),
        "a-note.md",
        &described("A note", "A line.", None),
    );
    std::fs::write(
        Memories::index(Some(folder.path())).expect("a project has an index"),
        "# Project memory\n",
    )
    .expect("failed to write note");
    note(folder.path(), "README", "Not a note.\n");
    note(folder.path(), ".draft.md", "Not a note either.\n");

    let memories = Memories::find(Some(folder.path()));

    assert_eq!(memories.all().len(), 1);
    assert_eq!(memories.all()[0].name(), "a-note");
    assert!(
        memories.diagnostics().is_empty(),
        "{:?}",
        memories.diagnostics()
    );
}

#[test]
fn invalid_entry_is_skipped_and_reported() {
    let folder = project();
    note(
        folder.path(),
        "good.md",
        &described("Good", "A line.", None),
    );
    note(folder.path(), "bare.md", "No front matter here.\n");

    let memories = Memories::find(Some(folder.path()));

    assert_eq!(memories.all().len(), 1);
    let trouble = memories
        .diagnostics()
        .iter()
        .find(|trouble| trouble.path().ends_with("bare.md"))
        .expect("it is remarked on");
    assert!(!trouble.is_loaded());
}

#[test]
fn rejects_about_outside_project() {
    let folder = project();
    note(
        folder.path(),
        "elsewhere.md",
        &described("Elsewhere", "Somewhere else.", Some("../other")),
    );

    let memories = Memories::find(Some(folder.path()));

    assert_eq!(memories.all().len(), 1);
    assert!(memories.all()[0].about().is_none());
    assert!(
        !memories.render().contains("../other"),
        "{}",
        memories.render()
    );
}
