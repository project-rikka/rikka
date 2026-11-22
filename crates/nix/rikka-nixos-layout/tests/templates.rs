use std::collections::BTreeSet;
use std::path::Path;

use rikka_nixos_layout::Module;

const TEMPLATE_MODULES: &str = "nix/dist/templates/configuration/rikka";

#[test]
fn template_modules_are_the_blank_layout() {
    let directory = Path::new(TEMPLATE_MODULES);
    let present: BTreeSet<String> = std::fs::read_dir(directory)
        .expect("the template is in the runfiles")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let expected: BTreeSet<String> = Module::EVERY
        .iter()
        .map(|module| module.file_name().to_owned())
        .collect();
    assert_eq!(present, expected);

    for module in Module::EVERY {
        let template = std::fs::read_to_string(directory.join(module.file_name())).unwrap();
        assert_eq!(template, module.blank().source(), "{}", module.file_name());
    }
}
