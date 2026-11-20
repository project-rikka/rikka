use std::{process::ExitCode, sync::Arc};

use installer_sys::{Command, DemoHost, DryRunHost, Environment, Host, Mode, SystemHost};
use installer_tui::{App, Log, Outcome};

const USAGE: &str = "\
usage: rikka-installer [options]

Installs NixOS on this machine: UEFI, GPT, systemd-boot, and an /etc/nixos
written as a flake.

  --demo       run against a made-up machine. No root, no disk touched.
  --dry-run    probe this machine, but run nothing that writes.
  --ascii      draw with ASCII only (automatic when TERM=linux).
  -h, --help   this help
  -V, --version

Without --demo or --dry-run this is the real thing and must run as root.
";

const TRANSCRIPT: &str = "/tmp/rikka-installer.log";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("rikka-installer: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = Mode::Live;
    let mut ascii = false;

    for argument in &arguments {
        match argument.as_str() {
            "--demo" => mode = Mode::Demo,
            "--dry-run" => mode = Mode::DryRun,
            "--ascii" => ascii = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("rikka-installer {}", env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
            other => return Err(format!("unknown option: {other}\n\n{USAGE}")),
        }
    }

    let host: Arc<dyn Host> = match mode {
        Mode::Live => Arc::new(SystemHost),
        Mode::DryRun => Arc::new(DryRunHost::default()),
        Mode::Demo => Arc::new(DemoHost::paced()),
    };

    let environment = Environment::probe(host.as_ref());
    report(&environment)?;

    let disks = installer_sys::disks(host.as_ref()).map_err(|error| error.to_string())?;
    let timezones = installer_sys::timezones(host.as_ref()).map_err(|error| error.to_string())?;

    let mut app = App::new(environment, disks, timezones);
    if ascii {
        app = app.plain();
    }

    let outcome =
        installer_tui::run(&mut app, Arc::clone(&host)).map_err(|error| error.to_string())?;
    finish(&app, outcome, host.as_ref())
}

fn report(environment: &Environment) -> Result<(), String> {
    let blockers = environment.blockers();

    if environment.mode.is_live() {
        let fatal: Vec<&str> = blockers
            .iter()
            .filter(|blocker| blocker.fatal)
            .map(|blocker| blocker.message.as_str())
            .collect();
        if !fatal.is_empty() {
            return Err(fatal.join("\n"));
        }
    }

    for blocker in &blockers {
        eprintln!("warning: {}", blocker.message);
    }
    Ok(())
}

fn finish(app: &App, outcome: Outcome, host: &dyn Host) -> Result<ExitCode, String> {
    let mode = host.mode();
    keep(host, app.install().log());

    let journal = host.journal();
    if !journal.is_empty() {
        println!("Commands this run would execute ({}):", journal.len());
        for entry in journal {
            println!("  {entry}");
        }
        println!();
    }

    match mode {
        Mode::Demo => println!("Demo run. Nothing was written to any disk."),
        Mode::DryRun => println!("Dry run. Nothing was written to any disk."),
        Mode::Live => {}
    }

    if outcome == Outcome::Reboot {
        if !mode.is_live() {
            println!("(a real run would reboot here)");
            return Ok(ExitCode::SUCCESS);
        }
        host.run(&Command::new("systemctl").arg("reboot"))
            .map_err(|error| error.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

fn keep(host: &dyn Host, log: &Log) {
    let mut contents = String::new();
    for line in log.lines() {
        contents.push_str(line);
        contents.push('\n');
    }
    if contents.is_empty() {
        return;
    }

    match host.write(TRANSCRIPT, &contents, 0o600) {
        Ok(()) if host.mode().is_live() => println!("This run's log is in {TRANSCRIPT}."),
        Ok(()) => {}
        Err(error) => {
            eprintln!("warning: failed to write {TRANSCRIPT}: {error}");
            print!("{contents}");
        }
    }
}
