use rikka_nix_protocol::{Action, ModuleFile, Request};
use rikka_nixos_layout::{Module, Modules, Packages};

#[test]
fn apply_carries_only_edited_modules_with_the_text_they_started_from() {
    let saved = Modules::blank();
    let mut edited = saved.clone();
    edited
        .get_mut(Module::Packages)
        .set("environment.systemPackages", Packages::new(["ripgrep"]))
        .expect("it takes it");

    let request = Request::apply(&saved, &edited, Action::Switch);

    assert_eq!(
        request,
        Request::Apply {
            files: vec![ModuleFile {
                module: Module::Packages,
                base: Module::Packages.blank().source().to_owned(),
                contents: edited.get(Module::Packages).source().to_owned(),
            }],
            action: Action::Switch,
        }
    );
}
