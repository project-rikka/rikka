use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

use rikka_nix::Generation;
use rikka_nix_protocol::{Action, ChangeState, ModuleFile, Request};

use crate::command::{self, Stream};
use crate::directory::Directory;
use crate::error::{Error, Result};
use crate::flake_lock::FlakeLock;
use crate::site::Site;

const DEFAULT_PROFILE: &str = "/nix/var/nix/profiles/default/bin";

const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";

const LOG_FORMAT: [&str; 2] = ["--log-format", "internal-json"];

const CONFIGURATION_NAMES: &str =
    r#"configurations: builtins.concatStringsSep "\n" (builtins.attrNames configurations)"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Preparing,
    Locking,
    Rebuilding,
    Activating,
}

pub trait Report {
    fn step(&mut self, step: Step);

    fn output(&mut self, line: &str);

    fn is_activating(&self) -> bool;
}

#[derive(Debug)]
pub enum Ending {
    Done { generation: Option<u32> },
    Failed { error: Error, changes: ChangeState },
}

pub fn execute(request: Request, site: &Site, report: &mut dyn Report) -> Ending {
    report.step(Step::Preparing);
    match request {
        Request::Apply { files, action } => apply(&files, action, site, report),
        Request::Rollback { generation } => rollback(generation, site, report),
        Request::Update { action } => update(action, site, report),
    }
}

fn apply(files: &[ModuleFile], action: Action, site: &Site, report: &mut dyn Report) -> Ending {
    let directory = match Directory::open(&site.modules) {
        Ok(directory) => directory,
        Err(source) => {
            return untouched(Error::Read {
                path: site.modules.display().to_string(),
                source,
            });
        }
    };
    if let Err(source) = directory.write(files) {
        let error = Error::Write {
            path: site.modules.display().to_string(),
            source,
        };
        return restored(error, directory.restore());
    }

    report.step(Step::Rebuilding);
    match rebuild(action, site, report) {
        Ok(()) => done(),
        Err(error) if reached_the_system(action, report) => modified(error),
        Err(error) => restored(error, directory.restore()),
    }
}

fn update(action: Action, site: &Site, report: &mut dyn Report) -> Ending {
    let lock = match FlakeLock::open(&site.flake) {
        Ok(lock) => lock,
        Err(source) => {
            return untouched(Error::Read {
                path: site.flake.join("flake.lock").display().to_string(),
                source,
            });
        }
    };

    report.step(Step::Locking);
    let mut command = program(site, "nix");
    command
        .args(["flake", "update", "--flake"])
        .arg(&site.flake)
        .args(LOG_FORMAT);
    if let Err(source) = lock.owner().run_as(&mut command) {
        return untouched(Error::Spawn {
            program: "nix".to_owned(),
            source,
        });
    }
    let outcome = run_reported(command, report).and_then(|()| {
        report.step(Step::Rebuilding);
        rebuild(action, site, report)
    });

    match outcome {
        Ok(()) => done(),
        Err(error) if reached_the_system(action, report) => modified(error),
        Err(error) => restored(error, lock.restore()),
    }
}

fn rollback(generation: u32, site: &Site, report: &mut dyn Report) -> Ending {
    report.step(Step::Activating);

    let mut switch = program(site, "nix-env");
    switch.args([
        "--profile",
        SYSTEM_PROFILE,
        "--switch-generation",
        &generation.to_string(),
    ]);
    if let Err(error) = run_reported(switch, report) {
        return untouched(error);
    }

    let activation = Generation::link(generation).join("bin/switch-to-configuration");
    let mut activate = program_at(site, &activation);
    activate.arg("switch");
    match run_reported(activate, report) {
        Ok(()) => done(),
        Err(error) => modified(error),
    }
}

fn rebuild(action: Action, site: &Site, report: &mut dyn Report) -> Result<()> {
    let flake = site.flake.display().to_string();
    let target = match configuration(site, report)? {
        Some(name) => format!("{flake}#{name}"),
        None => flake,
    };
    let mut command = program(site, "nixos-rebuild");
    command
        .args([action.argument(), "--flake", &target])
        .arg("--no-reexec")
        .args(LOG_FORMAT);
    run_reported(command, report)
}

fn configuration(site: &Site, report: &mut dyn Report) -> Result<Option<String>> {
    let mut command = program(site, "nix");
    command
        .args(["eval", "--raw"])
        .arg(format!("{}#nixosConfigurations", site.flake.display()))
        .args(["--apply", CONFIGURATION_NAMES])
        .args(LOG_FORMAT);
    let mut names = Vec::new();
    let status = command::run(command, &mut |stream, line| match stream {
        Stream::Stdout => names.push(line.to_owned()),
        Stream::Stderr => report.output(line),
    })?;
    succeeded("nix", status)?;
    match <[String; 1]>::try_from(names) {
        Ok([name]) => Ok(Some(name)),
        Err(_) => Ok(None),
    }
}

fn reached_the_system(action: Action, report: &dyn Report) -> bool {
    action.activates() && report.is_activating()
}

fn run_reported(command: Command, report: &mut dyn Report) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command::run(command, &mut |_, line| report.output(line))?;
    succeeded(&program, status)
}

fn succeeded(program: &str, status: ExitStatus) -> Result<()> {
    match status.success() {
        true => Ok(()),
        false => Err(Error::ExitStatus {
            program: program.to_owned(),
            status,
        }),
    }
}

fn program(site: &Site, name: &str) -> Command {
    program_at(site, &site.programs.join(name))
}

fn program_at(site: &Site, path: &Path) -> Command {
    let mut command = Command::new(path);
    command.env(
        "PATH",
        format!("{}:{DEFAULT_PROFILE}", site.programs.display()),
    );
    command
}

fn done() -> Ending {
    Ending::Done {
        generation: Generation::current(),
    }
}

fn untouched(error: Error) -> Ending {
    Ending::Failed {
        error,
        changes: ChangeState::Untouched,
    }
}

fn modified(error: Error) -> Ending {
    Ending::Failed {
        error,
        changes: ChangeState::Modified,
    }
}

fn restored(error: Error, restored: io::Result<()>) -> Ending {
    match restored {
        Ok(()) => untouched(error),
        Err(source) => Ending::Failed {
            error: Error::Restore {
                error: Box::new(error),
                source,
            },
            changes: ChangeState::Modified,
        },
    }
}
