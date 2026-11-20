use rikka_computer_use::ComputerUse;
use rikka_tool::json;
use rikka_tool_docs::{BASH, COMPUTER, TOOL_SEARCH, facts};
use rikka_tools::Toolbox;

fn named(stem: &str) -> String {
    facts()
        .expect("shipped documents should parse")
        .get(&format!("{stem}_tool"))
        .expect("every tool should have a name")
        .to_owned()
}

fn fitted(folder: &std::path::Path) -> Toolbox {
    Toolbox::builder(folder)
        .machine(ComputerUse::machine())
        .build()
        .expect("tools should be declared")
}

#[test]
fn computer_is_deferred_and_searchable() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let toolbox = fitted(folder.path());
    let declares = |stem: &str| {
        let name = named(stem);
        toolbox
            .functions()
            .iter()
            .any(|function| function.name() == name)
    };

    assert!(toolbox.deferred().is_hidden(&named(COMPUTER)));
    assert!(!toolbox.deferred().is_hidden(&named(BASH)));
    assert!(declares(COMPUTER));
    assert!(declares(TOOL_SEARCH));
}

#[tokio::test]
async fn invalid_batch_runs_nothing() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let toolbox = fitted(folder.path());
    let name = named(COMPUTER);
    let computer = toolbox
        .functions()
        .iter()
        .find(|function| function.name() == name)
        .expect("the computer is declared");

    for (actions, refusal) in [
        (json!([]), "no actions given"),
        (
            json!([{ "action": "start" }, { "action": "fly" }]),
            "action 2 (fly): unknown action \"fly\"",
        ),
        (
            json!([{ "action": "zoom", "region": [40, 40, 10, 10] }]),
            "action 1 (zoom): invalid region [40, 40, 10, 10]",
        ),
    ] {
        let refused = computer
            .call(json!({ "actions": actions }))
            .await
            .expect_err("an invalid batch should be refused");
        assert_eq!(refused, refusal);
    }
    let machine = toolbox.machine().expect("the toolbox has its machine");
    let computer = ComputerUse::computer_of(machine).expect("the machine is a computer");
    assert!(!computer.status().is_open());
}
