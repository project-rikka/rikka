use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::host;

const BUILD_ENVIRONMENT: &str = "rikka-gecko-build";

const PASSED_VARIABLES: [&str; 13] = [
    "HOME",
    "USER",
    "LOGNAME",
    "TERM",
    "LANG",
    "LC_ALL",
    "TZ",
    "XDG_CACHE_HOME",
    "RUSTUP_HOME",
    "CARGO_HOME",
    "MOZBUILD_STATE_PATH",
    "SCCACHE_DIR",
    "SCCACHE_CACHE_SIZE",
];

const RUST_TOOLCHAINS: &str = "taskcluster/kinds/toolchain/rust.yml";

const RUST_TOOLCHAIN: &str = "linux64-rust";

const ALIAS_KEYS: [&str; 2] = ["toolchain-alias:", "default:"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile {
    Dev,
    Release,
}

impl Profile {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Release => "release",
        }
    }
}

pub(crate) struct Mach {
    tree: PathBuf,
    mozconfig: PathBuf,
    objdir: PathBuf,
    profile: Profile,
    rust: Rust,
}

struct Rust {
    rustc: PathBuf,
    cargo: PathBuf,
    rustdoc: PathBuf,
}

impl Mach {
    pub(crate) fn new(
        tree: &Path,
        mozconfig: &Path,
        objdir: &Path,
        profile: Profile,
    ) -> Result<Mach> {
        Ok(Mach {
            tree: tree.to_owned(),
            mozconfig: mozconfig.to_owned(),
            objdir: objdir.to_owned(),
            profile,
            rust: Rust::install(&rust_channel(tree)?)?,
        })
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Result<()> {
        host::run(
            build_command()?
                .current_dir(&self.tree)
                .env("MOZCONFIG", &self.mozconfig)
                .env("MOZ_OBJDIR", &self.objdir)
                .env("RIKKA_GECKO_PROFILE", self.profile.name())
                .env("RUSTC", &self.rust.rustc)
                .env("CARGO", &self.rust.cargo)
                .env("RUSTDOC", &self.rust.rustdoc)
                .arg(self.tree.join("mach"))
                .arg("--no-interactive")
                .args(arguments),
        )
    }
}

impl Rust {
    fn install(channel: &str) -> Result<Rust> {
        host::run(build_command()?.args([
            "rustup",
            "toolchain",
            "install",
            channel,
            "--profile",
            "minimal",
        ]))?;
        let which = |tool: &str| -> Result<PathBuf> {
            let path = host::output(build_command()?.args([
                "rustup",
                "which",
                "--toolchain",
                channel,
                tool,
            ]))?;
            Ok(PathBuf::from(path.trim()))
        };
        Ok(Rust {
            rustc: which("rustc")?,
            cargo: which("cargo")?,
            rustdoc: which("rustdoc")?,
        })
    }
}

fn build_command() -> Result<Command> {
    let mut command = Command::new(host::require(BUILD_ENVIRONMENT)?);
    command.env_clear();
    for name in PASSED_VARIABLES {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    Ok(command)
}

fn rust_channel(tree: &Path) -> Result<String> {
    let path = tree.join(RUST_TOOLCHAINS);
    let toolchains = host::read(&path)?;
    let mut channel = None;
    let mut listing_aliases = false;
    for line in toolchains.lines() {
        if !line.starts_with(' ') && line.ends_with(':') {
            channel = None;
        }
        if line.contains("--channel") {
            channel = line.split('\'').nth(3);
        }
        let entry = line.trim();
        let aliased = match entry.strip_prefix("- ") {
            Some(item) => listing_aliases && item == RUST_TOOLCHAIN,
            None => {
                listing_aliases = ALIAS_KEYS.contains(&entry);
                ALIAS_KEYS
                    .iter()
                    .any(|key| entry.strip_prefix(key).map(str::trim) == Some(RUST_TOOLCHAIN))
            }
        };
        if aliased {
            return channel.map(str::to_owned).with_context(|| {
                format!(
                    "{} names {RUST_TOOLCHAIN} without a channel",
                    path.display()
                )
            });
        }
    }
    bail!(
        "found no channel for {RUST_TOOLCHAIN} in {}",
        path.display()
    )
}
