use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rikka_instance::TAG_ENV;

use crate::bazel::{self, Profile};
use crate::components::COMPONENTS;
use crate::shipped::{Contents, Program, SHARED};
use crate::{host, tree};

const TAG: &str = "dev";

const COMPOSITOR: &str = "rikka-compositor";

const DATA_DIRS_ENV: &str = "XDG_DATA_DIRS";

const DEFAULT_DATA_DIRS: &str = "/usr/local/share:/usr/share";

const SESSION_TREES: &str = ".bazel/session";

const STAGED: &str = "new";

pub(crate) fn compositor(workspace: &Path, profile: Profile, arguments: &[String]) -> Result<()> {
    let Some(mode) = display_mode(arguments) else {
        bail!("no Wayland session to nest in; run inside one or pass `-- --headless`");
    };

    let tree = workspace.join(SESSION_TREES).join(profile.name());
    let staged = tree.with_extension(STAGED);
    host::reset_dir(&staged)?;
    let session = session();
    let built = tree::build(workspace, profile, &session)?;
    let laid = tree::lay(workspace, &built, &session, &tree, &staged)?;
    tree::link_shared(&staged)?;
    host::remove(&tree)?;
    std::fs::rename(&staged, &tree)
        .with_context(|| format!("failed to move {} to {}", staged.display(), tree.display()))?;
    println!(
        "laid out session in {} ({} programs, {} data files)",
        tree.display(),
        laid.programs.len(),
        laid.data.len()
    );

    let compositor = tree.join(tree::BIN).join(COMPOSITOR);
    let data = data_dirs(&tree.join("share"))?;
    println!("\n===== starting {} =====", compositor.display());
    if mode == DisplayMode::Window {
        println!("  nested in this session with a private session bus; settings are shared");
    }
    host::exec(
        host::clean_command(&compositor)
            .args(arguments)
            .env(DATA_DIRS_ENV, data),
    )
}

fn session() -> Vec<Contents> {
    COMPONENTS
        .iter()
        .map(|component| component.session)
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DisplayMode {
    Window,
    Headless,
}

fn display_mode(arguments: &[String]) -> Option<DisplayMode> {
    let headless = arguments
        .iter()
        .any(|argument| argument == "--headless" || argument.starts_with("--headless="));
    if headless {
        return Some(DisplayMode::Headless);
    }
    let nests = std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("WAYLAND_SOCKET").is_some();
    nests.then_some(DisplayMode::Window)
}

fn data_dirs(share: &Path) -> Result<OsString> {
    let inherited = std::env::var_os(DATA_DIRS_ENV)
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| DEFAULT_DATA_DIRS.into());
    let dirs: Vec<PathBuf> = std::iter::once(share.to_path_buf())
        .chain(std::env::split_paths(&inherited).filter(|dir| dir != share))
        .collect();
    std::env::join_paths(dirs)
        .with_context(|| format!("failed to add {} to {DATA_DIRS_ENV}", share.display()))
}

pub(crate) fn app(
    workspace: &Path,
    named: &str,
    profile: Profile,
    arguments: &[String],
) -> Result<()> {
    let programs = || {
        COMPONENTS
            .iter()
            .flat_map(|component| component.session.programs)
    };
    let Some(application) = programs().find(|program| program.installed == named) else {
        let names: Vec<&str> = programs()
            .map(|program: &Program| program.installed)
            .collect();
        bail!(
            "unknown program `{named}`; expected one of {}",
            names.join(", ")
        );
    };
    let built = bazel::build(workspace, profile, [application.target])?;
    let program = built.program(application)?;

    println!("\n===== starting {} as {TAG} =====", program.display());
    println!("  app id `{named}-{TAG}`; settings and data are shared with the installed {named}");

    let mut command = host::clean_command(program);
    command.args(arguments).env(TAG_ENV, TAG);
    for shared in SHARED {
        command.env(shared.found_by, tree::supplied(shared.variable)?);
    }
    host::exec(&mut command)
}
