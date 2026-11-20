use std::ffi::OsStr;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const WITHHELD_VARIABLES: [&str; 6] = [
    "LD_LIBRARY_PATH",
    "TMPDIR",
    "TMP",
    "TEMP",
    "TEMPDIR",
    "NIX_BUILD_TOP",
];

pub(crate) fn which(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn require(program: &str) -> Result<PathBuf> {
    which(program).with_context(|| format!("`{program}` is not on PATH; run inside `nix develop`"))
}

pub(crate) fn clean_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    for name in WITHHELD_VARIABLES {
        command.env_remove(name);
    }
    command
}

pub(crate) fn exec(command: &mut Command) -> Result<()> {
    let failure = command.exec();
    Err(failure).with_context(|| {
        format!(
            "failed to start {}",
            Path::new(command.get_program()).display()
        )
    })
}

pub(crate) fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {}", describe(command)))?;
    if !status.success() {
        bail!("{} failed with {status}", describe(command));
    }
    Ok(())
}

pub(crate) fn output(command: &mut Command) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("failed to run {}", describe(command)))?;
    if !output.status.success() {
        bail!(
            "{} failed with {}\n{}",
            describe(command),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("{} wrote non-UTF-8 output", describe(command)))
}

pub(crate) fn rustfmt(source: &Path) -> Result<()> {
    run(Command::new(require("rustfmt")?)
        .args(["--edition", "2024"])
        .arg(source))
}

pub(crate) fn describe(command: &Command) -> String {
    let program = Path::new(command.get_program())
        .file_name()
        .unwrap_or(command.get_program())
        .to_string_lossy()
        .into_owned();
    std::iter::once(program)
        .chain(command.get_args().map(quote))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote(argument: &OsStr) -> String {
    let argument = argument.to_string_lossy();
    if argument.contains(' ') {
        format!("\"{argument}\"")
    } else {
        argument.into_owned()
    }
}

pub(crate) fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    create_parent(path)?;
    std::fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn copy(source: &Path, destination: &Path) -> Result<()> {
    create_parent(destination)?;
    let name = destination
        .file_name()
        .with_context(|| format!("{} has no file name", destination.display()))?;
    let mut staged_name = name.to_os_string();
    staged_name.push(format!(".new.{}", std::process::id()));
    let staged = destination.with_file_name(staged_name);
    let replace = || -> std::io::Result<()> {
        std::fs::copy(source, &staged)?;
        std::fs::rename(&staged, destination)
    };
    replace()
        .inspect_err(|_| {
            if let Err(error) = std::fs::remove_file(&staged) {
                eprintln!("warning: failed to remove {}: {error}", staged.display());
            }
        })
        .with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                destination.display()
            )
        })
}

pub(crate) fn reset_dir(path: &Path) -> Result<()> {
    remove(path)?;
    create_dir(path)
}

pub(crate) fn remove(path: &Path) -> Result<()> {
    let removed = match path.is_dir() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    };
    match removed {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("failed to remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

pub(crate) fn symlink(target: &Path, link: &Path) -> Result<()> {
    remove(link)?;
    create_parent(link)?;
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("failed to link {} to {}", link.display(), target.display()))
}

pub(crate) fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .permissions();
    permissions.set_mode(mode);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to set mode {mode:o} on {}", path.display()))
}

fn create_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}

pub(crate) fn create_parent(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    create_dir(parent)
}
