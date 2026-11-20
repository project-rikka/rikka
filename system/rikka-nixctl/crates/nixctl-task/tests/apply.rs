use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use nixctl_task::{Ending, Report, Site, Step, execute};
use rikka_nix::Module;
use rikka_nix_protocol::{Action, ChangeState, ModuleFile, Request};

const BEFORE: &str = "{ time.timeZone = \"Asia/Tokyo\"; }\n";
const AFTER: &str = "{ time.timeZone = \"Europe/Paris\"; }\n";
const ACTIVATING: &str = "activating";

#[derive(Default)]
struct Heard {
    lines: Vec<String>,
    activating: bool,
}

impl Report for Heard {
    fn step(&mut self, _: Step) {}

    fn output(&mut self, line: &str) {
        self.activating |= line == ACTIVATING;
        self.lines.push(line.to_owned());
    }

    fn is_activating(&self) -> bool {
        self.activating
    }
}

fn script(programs: &Path, name: &str, body: &str) {
    let script = programs.join(name);
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("it writes");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("it becomes executable");
}

fn site(folder: &Path, configurations: &str) -> Site {
    let site = Site {
        modules: folder.join("rikka"),
        flake: folder.to_owned(),
        programs: folder.join("bin"),
    };
    std::fs::create_dir_all(&site.modules).expect("it makes the modules folder");
    std::fs::create_dir_all(&site.programs).expect("it makes the programs folder");
    std::fs::write(site.modules.join(Module::System.file_name()), BEFORE).expect("it writes");
    script(&site.programs, "nix", &format!("printf '{configurations}'"));
    site
}

fn change(action: Action) -> Request {
    Request::Apply {
        files: vec![ModuleFile {
            module: Module::System,
            base: BEFORE.to_owned(),
            contents: AFTER.to_owned(),
        }],
        action,
    }
}

#[test]
fn failed_apply_restores_modules_only_before_activation() {
    let cases = [
        (Action::Switch, "building", ChangeState::Untouched, BEFORE),
        (Action::Switch, ACTIVATING, ChangeState::Modified, AFTER),
        (Action::Build, ACTIVATING, ChangeState::Untouched, BEFORE),
    ];
    for (action, says, expected, left) in cases {
        let folder = tempfile::tempdir().expect("it makes a folder");
        let site = site(folder.path(), "workstation");
        script(
            &site.programs,
            "nixos-rebuild",
            &format!("echo {says}\nexit 4"),
        );

        let ending = execute(change(action), &site, &mut Heard::default());

        let Ending::Failed { changes, .. } = ending else {
            panic!("{action:?} after `{says}` should fail");
        };
        assert_eq!(changes, expected, "{action:?} after `{says}`");
        assert_eq!(
            std::fs::read_to_string(site.modules.join(Module::System.file_name()))
                .expect("it reads"),
            left,
            "{action:?} after `{says}`"
        );
    }
}

#[test]
fn rebuild_follows_the_flakes_only_configuration() {
    let cases = [
        ("renamed", Some("renamed")),
        ("one\\ntwo", None),
        ("", None),
    ];
    for (configurations, chosen) in cases {
        let folder = tempfile::tempdir().expect("it makes a folder");
        let site = site(folder.path(), configurations);
        script(&site.programs, "nixos-rebuild", "echo \"$@\"");
        let mut heard = Heard::default();

        let ending = execute(change(Action::Switch), &site, &mut heard);

        assert!(matches!(ending, Ending::Done { .. }), "{ending:?}");
        let flake = site.flake.display().to_string();
        let target = match chosen {
            Some(name) => format!("{flake}#{name}"),
            None => flake,
        };
        assert_eq!(
            heard.lines,
            [format!(
                "switch --flake {target} --no-reexec --log-format internal-json"
            )],
            "{configurations:?}"
        );
    }
}
