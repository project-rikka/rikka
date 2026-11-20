use rikka_agent::{ToolKit, ToolKitSet};
use rikka_tools::{TOOL_SEARCH, Toolbox};

use crate::support::{Project, stranger};

#[test]
fn rejects_non_directory_root() {
    assert!(Toolbox::builder("/nowhere/at/all").build().is_err());
}

#[test]
fn deferred_tools_stay_available() {
    let strangers = ["mcp__github__create_issue", "mcp__linear__search"];
    let project = Project::fitted(|setup| {
        setup
            .deferred(strangers.map(|name| stranger(name, "Something somebody else wrote")))
            .tools([stranger("Skill", "Loads one of this project's skills")])
    });

    for name in strangers {
        assert!(project.toolbox().deferred().is_hidden(name), "{name}");
        assert!(project.function(name).is_some(), "{name}");
    }
    assert!(!project.toolbox().deferred().is_hidden("Skill"));
    assert!(project.function("Skill").is_some());

    let listed = project
        .toolbox()
        .searchable()
        .expect("deferred tools should be listed");
    for name in strangers {
        assert!(listed.contains(name), "{listed}");
    }
    assert!(project.declares(TOOL_SEARCH));
}

#[test]
fn app_tools_are_found_and_loaded_together() {
    let shelf = ToolKitSet::new();
    let project = Project::fitted(|setup| setup.toolkit_set(shelf.clone()));
    let toolbox = project.toolbox();
    assert!(toolbox.open_toolkit("Animate").is_none());

    shelf.replace([ToolKit::new("Animate")
        .with_instructions("Read the scene first.")
        .with_functions([
            stranger("Animate_Scene", "Describes the scene"),
            stranger("Animate_Pose", "Keys a pose"),
        ])]);

    let peeked = toolbox.toolkit("animate").expect("kit should be running");
    assert!(peeked.is_first());
    assert!(!toolbox.deferred().is_revealed("Animate_Pose"));

    let kit = toolbox
        .open_toolkit("animate")
        .expect("kit should be running");
    assert!(kit.is_first());
    assert_eq!(kit.toolkit().instructions(), "Read the scene first.");
    assert!(toolbox.deferred().is_revealed("Animate_Pose"));
    assert!(
        !toolbox
            .open_toolkit("Animate")
            .expect("kit should still be running")
            .is_first()
    );

    let afresh = toolbox.fresh().expect("toolbox should open again");
    assert!(!afresh.deferred().is_revealed("Animate_Pose"));
    assert!(
        afresh
            .open_toolkit("Animate")
            .expect("kit should be shared")
            .is_first()
    );
}
