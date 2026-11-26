use crate::{
    command::{Command, Output},
    error::Result,
    host::{Host, Mode, SystemHost},
    journal::Journal,
};

#[derive(Debug, Default)]
pub struct DryRunHost {
    inner: SystemHost,
    journal: Journal,
}

const HARMLESS: &[&str] = &[
    "lsblk",
    "timedatectl",
    "nixos-version",
    "ip",
    "find",
    "mktemp",
    "udevadm",
];

impl Host for DryRunHost {
    fn execute(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
        if HARMLESS.contains(&command.program.as_str()) {
            return self.inner.execute(command, on_line);
        }

        self.journal.command(command);
        let line = format!("(dry-run) not run: {command}");
        on_line(&line);
        Ok(Output {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn capture(&self, command: &Command) -> Result<String> {
        if HARMLESS.contains(&command.program.as_str()) {
            return self.inner.capture(command);
        }
        self.journal.command(command);
        Ok(String::new())
    }

    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }

    fn read(&self, path: &str) -> Result<String> {
        self.inner.read(path)
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        self.inner.read_bytes(path)
    }

    fn write(&self, path: &str, contents: &str, mode: u32) -> Result<()> {
        self.journal.write(path, contents, mode);
        Ok(())
    }

    fn mkdir(&self, path: &str, mode: u32) -> Result<()> {
        self.journal.mkdir(path, mode);
        Ok(())
    }

    fn which(&self, tool: &str) -> bool {
        self.inner.which(tool)
    }

    fn euid(&self) -> u32 {
        self.inner.euid()
    }

    fn mode(&self) -> Mode {
        Mode::DryRun
    }

    fn journal(&self) -> Vec<String> {
        self.journal.entries()
    }
}
