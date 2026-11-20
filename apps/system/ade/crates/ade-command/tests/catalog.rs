use ade_command::Commands;
use ade_scope::Scope;

use crate::support::{project, write};

#[test]
fn finds_every_command_template() {
    let folder = project();
    write(
        folder.path(),
        "review.md",
        "---\ndescription: Reviews a change.\nargument-hint: \"[path]\"\n---\n\nReview $1.\n",
    );
    write(
        folder.path(),
        "commit.md",
        "Write the commit message.\n\nSay why.\n",
    );
    write(folder.path(), "notes.txt", "not one");

    let commands = Commands::find(Some(folder.path()));

    let names: Vec<&str> = commands
        .all()
        .iter()
        .map(|command| command.name())
        .collect();
    assert_eq!(names, ["commit", "review"]);
    assert!(
        commands.diagnostics().is_empty(),
        "{:?}",
        commands.diagnostics()
    );
    let review = commands.get("review").unwrap();
    assert_eq!(review.scope(), Scope::Project);
    assert_eq!(review.description(), "Reviews a change.");
    assert_eq!(review.hint(), Some("[path]"));
    assert_eq!(review.body(), "Review $1.");
    let commit = commands.get("commit").unwrap();
    assert_eq!(commit.description(), "Write the commit message.");
    assert_eq!(commit.body(), "Write the commit message.\n\nSay why.");
}

#[test]
fn invalid_template_is_skipped() {
    let folder = project();
    write(folder.path(), "review.md", "Review it.\n");
    write(folder.path(), "broken.md", "---\ndescription: Reviews.\n");
    write(folder.path(), "Code Review.md", "Review it too.\n");

    let commands = Commands::find(Some(folder.path()));

    assert_eq!(commands.all().len(), 1);
    assert_eq!(commands.diagnostics().len(), 2);
    assert!(
        commands
            .diagnostics()
            .iter()
            .any(|trouble| trouble.path().ends_with("broken.md"))
    );
}
