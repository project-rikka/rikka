use std::process::{Child, Command, Stdio};

use crate::environment::Environment;
use crate::error::{Error, Result};

const SHELL: &str = "bash";

#[derive(Debug, Default)]
pub(crate) struct Launcher {
    children: Vec<Child>,
}

impl Launcher {
    pub(crate) fn launch(
        &mut self,
        command: &str,
        environment: Option<&Environment>,
        process_group: i32,
    ) -> Result<u32> {
        use std::os::unix::process::CommandExt as _;

        let command = command.trim();
        if command.is_empty() {
            return Err(Error::EmptyCommand);
        }
        self.reap();

        let mut shell = Command::new(SHELL);
        shell
            .arg("-lc")
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(process_group);
        if let Some(environment) = environment {
            environment.apply(&mut shell);
        }

        let child = shell.spawn().map_err(Error::Spawn)?;
        let id = child.id();
        self.children.push(child);
        Ok(id)
    }

    pub(crate) fn reap(&mut self) {
        self.children
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        self.reap();
    }
}
