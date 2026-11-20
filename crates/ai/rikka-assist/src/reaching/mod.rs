mod calling;
mod gist;
mod watch;

use rikka_sync::{Mutex, MutexGuard};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_seam::SeamClient;
use rikka_tool::{ToolKit, ToolKitSet};
use rikka_watch::Watcher;
use serde_json::Value;

use crate::protocol::{Described, exposed};

#[derive(Clone, Debug, Default)]
pub struct Apps {
    catalogue: Arc<Catalogue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    kit: String,
    title: String,
    about: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsedTool {
    app: String,
    tool: String,
    gist: Vec<String>,
}

#[derive(Debug, Default)]
struct Catalogue {
    shelf: ToolKitSet,
    state: Mutex<CatalogueState>,
    watcher: Mutex<Option<Watcher>>,
}

#[derive(Debug, Default)]
struct CatalogueState {
    reaching: HashSet<PathBuf>,
    running: Vec<Running>,
    used_tools: HashMap<String, UsedTool>,
}

#[derive(Debug)]
struct Running {
    path: PathBuf,
    app: App,
    kit: ToolKit,
    described: Described,
}

impl Apps {
    #[must_use]
    pub fn watch(shelf: ToolKitSet) -> Apps {
        let apps = Apps::new(shelf);
        watch::start(&apps.catalogue);
        apps
    }

    #[must_use]
    pub fn watch_dir(shelf: ToolKitSet, folder: &Path) -> Apps {
        let apps = Apps::new(shelf);
        watch::start_in(&apps.catalogue, folder.to_path_buf());
        apps
    }

    fn new(shelf: ToolKitSet) -> Apps {
        Apps {
            catalogue: Arc::new(Catalogue {
                shelf,
                ..Catalogue::default()
            }),
        }
    }

    #[must_use]
    pub fn running(&self) -> Vec<App> {
        self.catalogue
            .state()
            .running_apps()
            .into_iter()
            .map(|running| running.app.clone())
            .collect()
    }

    #[must_use]
    pub fn used_tool(&self, tool: &str) -> Option<UsedTool> {
        self.catalogue.state().used_tools.get(tool).cloned()
    }
}

impl App {
    #[must_use]
    pub fn new(kit: impl Into<String>, title: impl Into<String>, about: impl Into<String>) -> App {
        App {
            kit: kit.into(),
            title: title.into(),
            about: about.into(),
        }
    }
    #[must_use]
    pub fn kit(&self) -> &str {
        &self.kit
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn about(&self) -> &str {
        &self.about
    }
}

impl UsedTool {
    #[must_use]
    pub fn app(&self) -> &str {
        &self.app
    }

    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool
    }

    #[must_use]
    pub fn gist(&self, input: &Value) -> Option<String> {
        gist::gist(&self.gist, input)
    }
}

impl Catalogue {
    fn state(&self) -> MutexGuard<'_, CatalogueState> {
        self.state.lock()
    }

    fn begins_reaching(&self, path: &Path) -> bool {
        let mut state = self.state();
        let known = state.running.iter().any(|running| running.path == path);
        !known && state.reaching.insert(path.to_path_buf())
    }

    fn arrived(&self, path: &Path, described: Described, client: &SeamClient) {
        let mut state = self.state();
        state.reaching.remove(path);
        let known = state.running.iter().position(|known| known.path == path);
        if known.is_some_and(|at| state.running[at].described == described) {
            return;
        }
        for declared in &described.tools {
            state.used_tools.insert(
                exposed(&described.kit, &declared.name),
                UsedTool {
                    app: described.title.clone(),
                    tool: declared.name.clone(),
                    gist: declared.gist.clone(),
                },
            );
        }
        let running = Running {
            path: path.to_path_buf(),
            app: App::new(&described.kit, &described.title, &described.about),
            kit: calling::kit(&described, client),
            described,
        };
        match known {
            Some(at) => state.running[at] = running,
            None => state.running.push(running),
        }
        self.restock(&state);
    }

    fn left(&self, path: &Path) {
        let mut state = self.state();
        state.reaching.remove(path);
        state.running.retain(|running| running.path != path);
        self.restock(&state);
    }

    fn restock(&self, state: &CatalogueState) {
        self.shelf.replace(
            state
                .running_apps()
                .into_iter()
                .map(|running| running.kit.clone()),
        );
    }
}

impl CatalogueState {
    fn running_apps(&self) -> Vec<&Running> {
        let mut running_apps: Vec<&Running> = Vec::new();
        for running in &self.running {
            if running.kit.functions().is_empty() {
                continue;
            }
            if running_apps
                .iter()
                .any(|earlier| earlier.kit.name().eq_ignore_ascii_case(running.kit.name()))
            {
                tracing::debug!("{} is offered twice, keeping the first", running.kit.name());
                continue;
            }
            running_apps.push(running);
        }
        running_apps
    }
}
