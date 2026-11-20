use nixctl_task::Directory;
use rikka_nix::Module;
use rikka_nix_protocol::ModuleFile;

const THEIRS: &str = "{ time.timeZone = \"Asia/Tokyo\"; }\n";
const OURS: &str = "{ time.timeZone = \"Europe/Paris\"; }\n";

fn change(module: Module, base: &str) -> ModuleFile {
    ModuleFile {
        module,
        base: base.to_owned(),
        contents: OURS.to_owned(),
    }
}

#[test]
fn stale_names_a_module_that_moved_since_it_was_read() {
    let folder = tempfile::tempdir().expect("it makes a folder");
    std::fs::write(folder.path().join(Module::System.file_name()), THEIRS).expect("it writes");
    let directory = Directory::open(folder.path()).expect("it opens");
    let blank = |module: Module| module.blank().source().to_owned();

    let cases = [
        (change(Module::System, THEIRS), None),
        (
            change(Module::System, &blank(Module::System)),
            Some(Module::System),
        ),
        (change(Module::Boot, &blank(Module::Boot)), None),
        (change(Module::Boot, THEIRS), Some(Module::Boot)),
    ];
    for (file, stale) in cases {
        assert_eq!(
            directory.stale(std::slice::from_ref(&file)),
            stale,
            "{file:?}"
        );
    }
}

#[test]
fn write_touches_only_named_modules_and_fills_in_missing_ones() {
    let folder = tempfile::tempdir().expect("it makes a folder");
    let read = |module: Module| {
        std::fs::read_to_string(folder.path().join(module.file_name())).expect("it reads")
    };
    std::fs::write(folder.path().join(Module::System.file_name()), THEIRS).expect("it writes");
    let directory = Directory::open(folder.path()).expect("it opens");

    directory
        .write(&[change(Module::Packages, "")])
        .expect("it writes");

    assert_eq!(read(Module::System), THEIRS);
    assert_eq!(read(Module::Packages), OURS);
    assert_eq!(read(Module::Boot), Module::Boot.blank().source());
    assert_eq!(read(Module::Root), Module::Root.blank().source());
}
