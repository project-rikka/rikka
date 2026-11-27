use std::ffi::{CString, OsString};
use std::io::Write;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use rikka_web_crx::{Identity, Package, Version};
use rustix::io::{dup2, fcntl_dupfd_cloexec};
use rustix::pipe::{PipeFlags, pipe_with};

const CHROME: &str = "RIKKA_TEST_CHROME";

const EXTENSION: &str = "RIKKA_TEST_EXTENSION";

const DATA_DIR: &str = "--user-data-dir=";

const UNPACKED: &str = "rikka-extension";

const POLICIES: &str = "/etc/opt/chrome/policies";

const SAFE_FD: i32 = 16;

const COMMANDS_FD: i32 = 3;

const REPLIES_FD: i32 = 4;

const KEPT_COMMANDS_FD: i32 = 5;

const KEPT_REPLIES_FD: i32 = 6;

fn main() -> ExitCode {
    match run() {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("rikka-test-chrome: {error}");
            ExitCode::FAILURE
        }
    }
}

enum Never {}

fn run() -> Result<Never, String> {
    let chrome = std::env::var_os(CHROME).ok_or(format!("{CHROME} is unset"))?;
    let extension = std::env::var_os(EXTENSION).ok_or(format!("{EXTENSION} is unset"))?;
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let data_dir = arguments
        .iter()
        .find_map(|argument| argument.to_str()?.strip_prefix(DATA_DIR).map(PathBuf::from))
        .ok_or("Chrome was started without --user-data-dir")?;
    let unpacked = data_dir.join(UNPACKED);
    let zip = std::fs::read(&extension)
        .map_err(|error| format!("failed to read the extension: {error}"))?;
    Package::from_zip(&zip)
        .and_then(|package| {
            package.stamp(&Identity::rikka(), Version::parse("1.0").expect("valid"))
        })
        .and_then(|package| package.unpack(&unpacked))
        .map_err(|error| format!("failed to unpack the extension: {error}"))?;
    let (commands_read, commands_write) =
        pipe_with(PipeFlags::CLOEXEC).map_err(|error| error.to_string())?;
    let (replies_read, replies_write) =
        pipe_with(PipeFlags::CLOEXEC).map_err(|error| error.to_string())?;
    let mut command = serde_json::to_vec(&serde_json::json!({
        "id": 1,
        "method": "Extensions.loadUnpacked",
        "params": { "path": unpacked },
    }))
    .expect("the command encodes");
    command.push(0);
    std::fs::File::from(
        commands_write
            .try_clone()
            .map_err(|error| error.to_string())?,
    )
    .write_all(&command)
    .map_err(|error| format!("failed to queue the extension load: {error}"))?;
    let lifted: Vec<OwnedFd> = [
        &commands_read,
        &replies_write,
        &commands_write,
        &replies_read,
    ]
    .into_iter()
    .map(|fd| fcntl_dupfd_cloexec(fd, SAFE_FD))
    .collect::<Result<_, _>>()
    .map_err(|error| error.to_string())?;
    for (fd, target) in
        lifted
            .iter()
            .zip([COMMANDS_FD, REPLIES_FD, KEPT_COMMANDS_FD, KEPT_REPLIES_FD])
    {
        let mut placed = unsafe { OwnedFd::from_raw_fd(target) };
        let duplicated = dup2(fd, &mut placed);
        std::mem::forget(placed);
        duplicated.map_err(|error| format!("failed to place fd {target}: {error}"))?;
    }
    hide_policies()?;
    let error = Command::new(chrome)
        .args(arguments)
        .arg("--remote-debugging-pipe")
        .arg("--enable-unsafe-extension-debugging")
        .exec();
    Err(format!("failed to start Chrome: {error}"))
}

fn hide_policies() -> Result<(), String> {
    if !Path::new(POLICIES).is_dir() {
        return Ok(());
    }
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    if unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) } != 0 {
        return Err(format!(
            "failed to enter a namespace: {}",
            std::io::Error::last_os_error()
        ));
    }
    for (file, contents) in [
        ("/proc/self/uid_map", format!("{uid} {uid} 1")),
        ("/proc/self/setgroups", "deny".to_owned()),
        ("/proc/self/gid_map", format!("{gid} {gid} 1")),
    ] {
        std::fs::write(file, contents)
            .map_err(|error| format!("failed to write {file}: {error}"))?;
    }
    let root = CString::new("/").expect("no interior nul");
    let tmpfs = CString::new("tmpfs").expect("no interior nul");
    let policies = CString::new(POLICIES).expect("no interior nul");
    let private = unsafe {
        libc::mount(
            std::ptr::null(),
            root.as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    };
    let hidden = private == 0
        && unsafe {
            libc::mount(
                tmpfs.as_ptr(),
                policies.as_ptr(),
                tmpfs.as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                std::ptr::null(),
            )
        } == 0;
    if !hidden {
        return Err(format!(
            "failed to hide the machine's Chrome policies: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
