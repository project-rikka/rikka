use std::path::PathBuf;

use rikka_agent::{CompactionPrompts, ToolKitSet};
use rikka_computer_use::ComputerUse;
use rikka_mcp::{Hub, Place, Vault};
use rikka_seam::SeamClient;
use rikka_thread::{Assistant, Store};
use rikka_tools::Toolbox;
use rikka_ui::{Label, tr};

use crate::prompts::Prompts;

const KEPT: &str = "rikka-shell/assistant";

const SERVERS: &str = "assistant/mcp.json";

const PLACE: &str = "assistant";

const PLACE_ABOUT: &str =
    "the desktop assistant's own servers, offered in every conversation of its sidebar";

#[derive(Debug, thiserror::Error)]
pub(crate) enum SetupError {
    #[error(transparent)]
    Prompts(#[from] rikka_prompt::Error),

    #[error(transparent)]
    Tools(#[from] rikka_tools::Error),

    #[error(transparent)]
    Thread(#[from] rikka_thread::Error),

    #[error("no home directory for the assistant")]
    Homeless,
}

pub(crate) fn fit(
    prompts: &Prompts,
    seam: SeamClient,
    language: &str,
    shelf: ToolKitSet,
    hub: &Hub,
) -> Result<Assistant, SetupError> {
    let home = home().ok_or(SetupError::Homeless)?;
    let facts = prompts.facts(&home, language)?;

    let toolbox = hub
        .equip(
            Toolbox::builder(&home)
                .machine(ComputerUse::desktop(seam))
                .web()
                .toolkit_set(shelf),
        )
        .build()?;

    Ok(Assistant::builder(toolbox)
        .instruction(prompts.system(&facts)?)
        .instruction(hub.instruction())
        .instruction(prompts.environment(&facts)?)
        .compaction_prompts(CompactionPrompts::new(prompts.compact(&facts)?))
        .build()?)
}

#[must_use]
pub(super) fn servers() -> Hub {
    let mut servers = Hub::builder().vault(Vault::from_runtime_dir());
    if let Ok(paths) = rikka_dirs::Paths::resolve() {
        let file = paths.config_dir().join(SERVERS);
        servers = servers.place(Place::new(PLACE, file).with_description(PLACE_ABOUT));
    }
    if let Some(home) = home() {
        servers = servers.folder(home);
    }
    servers.build()
}

pub(super) fn place_titled(_place: &str) -> Label {
    tr!("shell.assistant.place")
}

#[must_use]
pub(crate) fn home() -> Option<PathBuf> {
    rikka_dirs::home().map(std::path::Path::to_path_buf)
}

#[must_use]
pub(crate) fn store() -> Option<Store> {
    let folder = rikka_dirs::data_home()?.join(KEPT);
    Store::open(&folder)
        .inspect_err(|error| tracing::warn!("assistant cannot store conversations: {error}"))
        .ok()
}

pub(crate) fn spawn(work: impl std::future::Future<Output = ()> + Send + 'static) {
    drop(rikka_ui::work_runtime().spawn(work));
}

pub(crate) fn spawn_blocking(work: impl FnOnce() + Send + 'static) {
    drop(rikka_ui::work_runtime().spawn_blocking(work));
}
