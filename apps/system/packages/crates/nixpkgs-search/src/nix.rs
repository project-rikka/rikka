use std::ffi::OsStr;
use std::io::{self, Read};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};

use crate::cancel::Cancel;
use crate::error::{Error, Result};

const NIXPKGS: &str = "nixpkgs";

const FEATURES: [&str; 2] = ["--extra-experimental-features", "nix-command flakes"];

const QUIET: &str = "--quiet";

pub(crate) fn installable(system: &str) -> String {
    format!("{NIXPKGS}#legacyPackages.{system}")
}

pub(crate) fn run<S: AsRef<OsStr>>(arguments: &[S], cancel: &Cancel) -> Result<Vec<u8>> {
    if cancel.is_stopped() {
        return Err(Error::Cancelled);
    }
    let mut child = Command::new("nix")
        .args(FEATURES)
        .arg(QUIET)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::Spawn)?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    cancel.track(child);
    let output = read_output(stdout, stderr);

    let Some(mut child) = cancel.take_child() else {
        return Err(Error::Cancelled);
    };
    let status = child.wait().map_err(Error::Spawn)?;
    let (stdout, stderr) = output.map_err(Error::Output)?;

    if cancel.is_stopped() {
        return Err(Error::Cancelled);
    }
    if !status.success() {
        return Err(Error::Failed(error_message(&stderr)));
    }
    Ok(stdout)
}

fn read_output(stdout: ChildStdout, stderr: ChildStderr) -> io::Result<(Vec<u8>, Vec<u8>)> {
    std::thread::scope(|scope| {
        let stderr = std::thread::Builder::new()
            .name("rikka-nix-stderr".to_owned())
            .spawn_scoped(scope, move || read_all(stderr))?;
        let stdout = read_all(stdout)?;
        let stderr = stderr.join().expect("stderr reader panicked")?;
        Ok((stdout, stderr))
    })
}

fn read_all(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn error_message(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let lines = || {
        stderr
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
    };
    lines()
        .filter_map(|line| line.strip_prefix("error:"))
        .map(str::trim)
        .find(|message| !message.is_empty())
        .or_else(|| lines().next())
        .unwrap_or("nix produced no output")
        .to_owned()
}

pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' | '"' => {
                out.push('\\');
                out.push(ch);
            }
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
