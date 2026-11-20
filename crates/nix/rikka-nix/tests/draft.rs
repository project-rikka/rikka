use rikka_nix::{Action, Draft, Module, Modules, Packages, Request};

const PACKAGES: &str = "environment.systemPackages";
const ZONE: &str = "time.timeZone";

fn installed(names: &[&str]) -> Modules {
    let mut modules = Modules::blank();
    modules
        .get_mut(Module::Packages)
        .set(PACKAGES, Packages::new(names.iter().copied()))
        .expect("it takes it");
    modules
}

#[test]
fn rebase_keeps_edits_and_follows_modules_changed_elsewhere() {
    let mut draft = Draft::new(Modules::blank());
    draft
        .edited_mut(Module::System)
        .set(ZONE, "Asia/Tokyo")
        .expect("it takes it");

    let overridden = draft.rebase(installed(&["firefox"]));

    assert_eq!(overridden, []);
    let Request::Apply { files, .. } = draft.request(Action::Switch) else {
        panic!("it applies");
    };
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].module, Module::System);
    assert_eq!(files[0].base, Module::System.blank().source());
    assert!(files[0].contents.contains("Asia/Tokyo"));
    assert!(draft.edited(Module::Packages).source().contains("firefox"));
}

#[test]
fn rebase_names_modules_edited_on_both_sides() {
    let mut draft = Draft::new(Modules::blank());
    draft
        .edited_mut(Module::Packages)
        .set(PACKAGES, Packages::new(["ripgrep"]))
        .expect("it takes it");

    let overridden = draft.rebase(installed(&["firefox"]));

    assert_eq!(overridden, [Module::Packages]);
    assert!(!draft.is_changed());
    assert!(draft.edited(Module::Packages).source().contains("firefox"));
}
