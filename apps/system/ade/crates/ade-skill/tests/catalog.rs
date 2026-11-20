use std::path::Path;

use ade_scope::Scope;
use ade_skill::Skills;

fn project() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::create_dir_all(folder.path().join(".rikka/skills")).expect("made");
    folder
}

fn write(project: &Path, name: &str, text: &str) {
    let at = project.join(".rikka/skills").join(name);
    std::fs::create_dir_all(&at).expect("made");
    std::fs::write(at.join("SKILL.md"), text).expect("failed to write skill");
}

fn skill(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\nDo the thing.\n")
}

#[test]
fn finds_every_skill_by_name() {
    let folder = project();
    write(folder.path(), "review", &skill("review", "Reviews code."));
    write(folder.path(), "commit", &skill("commit", "Writes commits."));

    let skills = Skills::find(Some(folder.path()));

    assert_eq!(skills.names().collect::<Vec<_>>(), ["commit", "review"]);
    assert_eq!(skills.get("review").unwrap().scope(), Scope::Project);
    assert!(
        skills.diagnostics().is_empty(),
        "{:?}",
        skills.diagnostics()
    );
}

#[test]
fn duplicate_skill_names_keep_one() {
    let folder = project();
    write(folder.path(), "review", &skill("review", "Reviews code."));
    write(
        folder.path(),
        "reviewing",
        &skill("review", "Reviews it too."),
    );

    let skills = Skills::find(Some(folder.path()));

    assert_eq!(skills.all().len(), 1);
    assert_eq!(skills.get("review").unwrap().description(), "Reviews code.");
    assert!(
        skills
            .diagnostics()
            .iter()
            .any(|trouble| !trouble.is_loaded() && trouble.message().contains("already a skill"))
    );
}

#[test]
fn invalid_skill_is_skipped() {
    let folder = project();
    write(folder.path(), "review", &skill("review", "Reviews code."));
    write(folder.path(), "broken", "no front matter at all\n");

    let skills = Skills::find(Some(folder.path()));

    assert_eq!(skills.names().collect::<Vec<_>>(), ["review"]);
    assert_eq!(skills.diagnostics().len(), 1);
    assert!(skills.diagnostics()[0].message().contains("---"));
}

#[test]
fn lists_skill_resources_without_hidden() {
    let folder = project();
    write(folder.path(), "pdf", &skill("pdf", "Reads PDFs."));
    let bundle = folder.path().join(".rikka/skills/pdf");
    for (file, text) in [
        ("scripts/extract.py", ""),
        ("references/REFERENCE.md", ""),
        (".git/HEAD", ""),
        (".gitignore", ""),
    ] {
        let at = bundle.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).expect("made");
        std::fs::write(at, text).expect("failed to write skill");
    }

    let skills = Skills::find(Some(folder.path()));
    let resources = skills.get("pdf").unwrap().resources();

    assert_eq!(
        resources.files(),
        ["references/REFERENCE.md", "scripts/extract.py"]
    );
    assert!(!resources.is_partial());
}
