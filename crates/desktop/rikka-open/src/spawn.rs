use std::io::ErrorKind;
use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};

pub fn spawn_detached(command: &mut Command) -> Result<u32> {
    let program = command.get_program().to_string_lossy().into_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(command);
    let child = command.spawn().map_err(|source| match source.kind() {
        ErrorKind::NotFound => Error::NotInstalled {
            program: program.clone(),
        },
        _ => Error::Spawn {
            program: program.clone(),
            source,
        },
    })?;
    let pid = child.id();
    release(child);
    Ok(pid)
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(unix)]
fn release(child: Child) {
    crate::reaper::reap(child);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    use windows::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    command.creation_flags(DETACHED_PROCESS.0 | CREATE_NEW_PROCESS_GROUP.0);
}

#[cfg(windows)]
fn release(child: Child) {
    drop(child);
}
