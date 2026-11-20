use rikka_sync::Mutex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures_util::Stream;
use rikka_agent::{Function, ToolKitSet};
use rikka_tools::ToolboxBuilder;
use rikka_watch::{Event, Reach};
use tokio::sync::{mpsc, watch};

use crate::catalog::Catalog;
use crate::contents::Contents;
use crate::error::{Error, Result};
use crate::mcp::Mcp;
use crate::place::Place;
use crate::prompt::Prompt;
use crate::prompted::Prompted;
use crate::resource;
use crate::server::Server;
use crate::server_status::ServerStatus;
use crate::session::{Ground, Session};
use crate::sign_in;
use crate::transport::Transport;
use crate::vault::Vault;
use crate::watch::Watch;
use crate::{instruction, resources_tool, tool};

const RELOAD_DEBOUNCE: Duration = Duration::from_millis(150);

#[must_use]
pub struct HubBuilder {
    places: Vec<Place>,
    folder: Option<PathBuf>,
    vault: Option<Vault>,
}

impl std::fmt::Debug for HubBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HubBuilder")
            .field("places", &self.places)
            .field("folder", &self.folder)
            .finish_non_exhaustive()
    }
}

impl HubBuilder {
    pub fn place(mut self, place: Place) -> HubBuilder {
        self.places.push(place);
        self
    }

    pub fn folder(mut self, folder: impl Into<PathBuf>) -> HubBuilder {
        self.folder = Some(folder.into());
        self
    }

    pub fn vault(mut self, vault: Option<Vault>) -> HubBuilder {
        self.vault = vault;
        self
    }

    pub fn build(self) -> Hub {
        let inner = Arc::new(Inner {
            state: Mutex::new(HubState {
                mcp: Arc::new(Mcp::default()),
                read: vec![Catalog::new(); self.places.len()],
                declared: Catalog::new(),
                origins: BTreeMap::new(),
                troubles: Vec::new(),
                signing: BTreeSet::new(),
            }),
            places: self.places,
            folder: self.folder,
            vault: self.vault,
            shelf: ToolKitSet::new(),
            watch: Watch::default(),
            turn: tokio::sync::Mutex::new(()),
            beats: watch::Sender::new(0),
            watcher: Mutex::new(None),
        });

        let restocking = Arc::downgrade(&inner);
        inner.watch.set(move |_| {
            if let Some(inner) = restocking.upgrade() {
                inner.restock();
                inner.beat();
            }
        });
        Hub { inner }
    }
}

#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
}

struct Inner {
    places: Vec<Place>,
    folder: Option<PathBuf>,
    vault: Option<Vault>,
    shelf: ToolKitSet,
    watch: Watch,
    state: Mutex<HubState>,
    turn: tokio::sync::Mutex<()>,
    beats: watch::Sender<u64>,
    watcher: Mutex<Option<rikka_watch::Watcher>>,
}

struct HubState {
    mcp: Arc<Mcp>,
    read: Vec<Catalog>,
    declared: Catalog,
    origins: BTreeMap<String, usize>,
    troubles: Vec<String>,
    signing: BTreeSet<String>,
}

impl Hub {
    pub fn builder() -> HubBuilder {
        HubBuilder {
            places: Vec::new(),
            folder: None,
            vault: None,
        }
    }

    #[must_use]
    pub fn places(&self) -> &[Place] {
        &self.inner.places
    }

    #[must_use]
    pub fn place(&self, name: &str) -> Option<&Place> {
        self.inner.places.iter().find(|place| place.name() == name)
    }

    #[must_use]
    pub fn toolkit_set(&self) -> ToolKitSet {
        self.inner.shelf.clone()
    }

    pub async fn start(&self) {
        self.settle(BTreeSet::new()).await;
        self.watch_places();
    }

    pub fn catalog(&self, place: &str) -> Result<Catalog> {
        let place = self.located(place)?;
        Catalog::read(place.path())
    }

    pub async fn save_catalog(&self, place: &str, catalog: Catalog) -> Result<()> {
        let path = self.located(place)?.path().to_path_buf();
        tokio::task::spawn_blocking(move || catalog.write(&path))
            .await
            .map_err(|error| Error::Unwritable {
                path: place.to_owned(),
                reason: error.to_string(),
            })??;
        self.settle(BTreeSet::new()).await;
        Ok(())
    }

    pub async fn refresh(&self) {
        self.settle(BTreeSet::new()).await;
    }

    pub async fn reconnect(&self, server: &str) -> Result<()> {
        self.declared(server)?;
        self.settle(BTreeSet::from([server.to_owned()])).await;
        Ok(())
    }

    #[must_use]
    pub fn statuses(&self) -> Vec<ServerStatus> {
        let state = self.inner.state();
        state
            .mcp
            .statuses()
            .into_iter()
            .map(|standing| {
                let place = state
                    .origins
                    .get(standing.server())
                    .and_then(|at| self.inner.places.get(*at))
                    .map(Place::name);
                standing.with_place(place)
            })
            .collect()
    }

    #[must_use]
    pub fn status(&self, server: &str) -> Option<ServerStatus> {
        self.statuses()
            .into_iter()
            .find(|standing| standing.server() == server)
    }

    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        self.inner.state().troubles.clone()
    }

    #[must_use]
    pub fn is_signing_in(&self, server: &str) -> bool {
        self.inner.state().signing.contains(server)
    }

    #[must_use]
    pub fn is_signed_in(&self, server: &str) -> bool {
        let Some(vault) = &self.inner.vault else {
            return false;
        };
        self.reached_over_http(server)
            .is_some_and(|url| vault.contains(&url))
    }

    pub fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        let receiver = self.inner.beats.subscribe();
        futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.changed().await.ok()?;
            receiver.borrow_and_update();
            Some(((), receiver))
        })
    }

    pub async fn probe(&self, server: Server) -> ServerStatus {
        let ground = self.inner.ground();
        let catalog = Catalog::from_servers([server.with_disabled(false)]);
        let mcp = Mcp::connect(
            &catalog,
            &ground,
            &Watch::default(),
            &Mcp::default(),
            &BTreeSet::new(),
        )
        .await;
        mcp.statuses()
            .into_iter()
            .next()
            .expect("single-server catalog should hold that server")
    }

    pub async fn sign_in(&self, server: &str) -> Result<()> {
        let declared = self.declared(server)?;
        let expanded = declared
            .expanded(|name| std::env::var(name).ok())
            .map_err(|unset| Error::SignIn {
                server: server.to_owned(),
                reason: unset.to_string(),
            })?;
        let Transport::Http(http) = expanded.transport() else {
            return Err(Error::Local(server.to_owned()));
        };
        let vault = self.inner.vault.clone().ok_or_else(|| Error::SignIn {
            server: server.to_owned(),
            reason: "there is nowhere to keep a sign-in".to_owned(),
        })?;
        let challenge = self
            .status(server)
            .and_then(|standing| standing.challenge().map(str::to_owned));

        self.inner.state().signing.insert(server.to_owned());
        self.inner.beat();
        let signed = sign_in::sign_in(http, &vault, challenge.as_deref()).await;
        self.inner.state().signing.remove(server);

        self.settle(BTreeSet::from([server.to_owned()])).await;
        signed.map_err(|reason| Error::SignIn {
            server: server.to_owned(),
            reason,
        })
    }

    pub async fn sign_out(&self, server: &str) -> Result<()> {
        self.declared(server)?;
        if let (Some(vault), Some(url)) = (&self.inner.vault, self.reached_over_http(server)) {
            vault.remove(&url).await.map_err(|error| Error::SignOut {
                server: server.to_owned(),
                reason: error.to_string(),
            })?;
        }
        self.settle(BTreeSet::from([server.to_owned()])).await;
        Ok(())
    }

    #[must_use]
    pub fn instruction(&self) -> String {
        instruction::instruction(self)
    }

    #[must_use]
    pub fn tool(&self) -> Function {
        tool::declare(self)
    }

    #[must_use]
    pub fn resources_tool(&self) -> Function {
        resources_tool::declare(self)
    }

    #[must_use]
    pub fn equip(&self, toolbox: ToolboxBuilder) -> ToolboxBuilder {
        toolbox
            .toolkit_set(self.toolkit_set())
            .deferred([self.resources_tool(), self.tool()])
    }

    #[must_use]
    pub fn prompt(&self, command: &str) -> Option<Prompt> {
        self.prompted_by(command).map(|(_, prompt)| prompt)
    }

    pub async fn get_prompt(&self, command: &str, values: &[String]) -> Result<Prompted> {
        let (session, prompt) = self
            .prompted_by(command)
            .ok_or_else(|| Error::Unprompted(command.to_owned()))?;
        let arguments = prompt.filled(values)?;
        session
            .prompted(prompt.name(), arguments)
            .await
            .map_err(|reason| unanswered(&session, reason))
    }

    pub async fn complete(
        &self,
        command: &str,
        given: &[String],
        typed: &str,
    ) -> Result<Vec<String>> {
        let (session, prompt) = self
            .prompted_by(command)
            .ok_or_else(|| Error::Unprompted(command.to_owned()))?;
        let Some(argument) = prompt.arguments().get(given.len()) else {
            return Ok(Vec::new());
        };
        let context = prompt
            .arguments()
            .iter()
            .zip(given)
            .map(|(argument, value)| (argument.name().to_owned(), value.clone()))
            .collect();
        session
            .complete(prompt.name(), argument.name(), typed, context)
            .await
            .map_err(|reason| unanswered(&session, reason))
    }

    #[must_use]
    pub fn is_resource_mention(&self, mention: &str) -> bool {
        self.mentioned(mention).is_some()
    }

    pub async fn read_mention(&self, mention: &str) -> Result<Vec<Contents>> {
        let (session, uri) = self
            .mentioned(mention)
            .ok_or_else(|| Error::Unmentioned(mention.to_owned()))?;
        session
            .read(&uri)
            .await
            .map_err(|reason| unanswered(&session, reason))
    }

    pub async fn read(&self, server: &str, uri: &str) -> Result<Vec<Contents>> {
        let session = self.inner.mcp().named(server).cloned();
        let Some(session) = session else {
            self.declared(server)?;
            return Err(Error::Disconnected(server.to_owned()));
        };
        if !session.reads() {
            return Err(Error::Resourceless(server.to_owned()));
        }
        session
            .read(uri)
            .await
            .map_err(|reason| unanswered(&session, reason))
    }

    fn prompted_by(&self, command: &str) -> Option<(Arc<Session>, Prompt)> {
        self.inner.mcp().sessions().find_map(|session| {
            let prompt = session
                .listing()
                .prompts
                .iter()
                .find(|prompt| prompt.command() == command)?
                .clone();
            Some((Arc::clone(session), prompt))
        })
    }

    fn mentioned(&self, mention: &str) -> Option<(Arc<Session>, String)> {
        let (label, uri) = resource::split(mention)?;
        let session = self
            .inner
            .mcp()
            .labelled(label)
            .filter(|session| session.reads())
            .cloned()?;
        Some((session, uri.to_owned()))
    }

    pub(crate) fn declared(&self, server: &str) -> Result<Server> {
        self.inner
            .state()
            .declared
            .get(server)
            .cloned()
            .ok_or_else(|| Error::Unknown(server.to_owned()))
    }

    pub(crate) fn declared_in(&self, server: &str) -> Vec<&Place> {
        self.inner
            .places
            .iter()
            .filter(|place| {
                Catalog::read(place.path()).is_ok_and(|catalog| catalog.get(server).is_some())
            })
            .collect()
    }

    fn located(&self, place: &str) -> Result<&Place> {
        self.place(place)
            .ok_or_else(|| Error::Placeless(place.to_owned()))
    }

    fn reached_over_http(&self, server: &str) -> Option<String> {
        let declared = self.declared(server).ok()?;
        let expanded = declared.expanded(|name| std::env::var(name).ok()).ok()?;
        match expanded.transport() {
            Transport::Http(http) => Some(http.url().to_owned()),
            Transport::Stdio(_) => None,
        }
    }

    async fn settle(&self, again: BTreeSet<String>) {
        let _turn = self.inner.turn.lock().await;

        let paths: Vec<PathBuf> = self
            .inner
            .places
            .iter()
            .map(|place| place.path().to_path_buf())
            .collect();
        let read = tokio::task::spawn_blocking(move || {
            paths
                .iter()
                .map(|path| Catalog::read(path))
                .collect::<Vec<Result<Catalog>>>()
        })
        .await
        .unwrap_or_default();

        let (declared, retained) = {
            let mut state = self.inner.state();
            state.troubles.clear();
            for (at, read) in read.into_iter().enumerate() {
                match read {
                    Ok(catalog) => state.read[at] = catalog,
                    Err(error) => state.troubles.push(error.to_string()),
                }
            }
            let declared = Catalog::merge(state.read.iter().cloned());
            state.origins = origins(&state.read);
            if declared == state.declared && again.is_empty() {
                drop(state);
                self.inner.beat();
                return;
            }
            (declared, Arc::clone(&state.mcp))
        };

        let ground = self.inner.ground();
        let mcp = Mcp::connect(&declared, &ground, &self.inner.watch, &retained, &again).await;
        {
            let mut state = self.inner.state();
            state.mcp = Arc::new(mcp);
            state.declared = declared;
        }
        self.inner.restock();
        self.inner.beat();
    }

    fn watch_places(&self) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("MCP declarations are not watched without a Tokio runtime");
            return;
        };
        let watched: Vec<PathBuf> = self
            .inner
            .places
            .iter()
            .map(|place| place.path().to_path_buf())
            .collect();
        let folders: BTreeSet<PathBuf> = watched
            .iter()
            .filter_map(|path| path.parent().map(PathBuf::from))
            .collect();

        let (changed, mut changes) = mpsc::unbounded_channel::<()>();
        let watcher = rikka_watch::Watcher::spawn(move |event| {
            let declared = match event {
                Event::Changed(change) => watched.contains(&change.path),
                Event::Lost => true,
            };
            if declared {
                let _ = changed.send(());
            }
        });
        let watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::warn!("failed to watch MCP declarations: {error}");
                return;
            }
        };
        for folder in &folders {
            if let Err(error) = std::fs::create_dir_all(folder) {
                tracing::warn!("failed to create {}: {error}", folder.display());
            } else if let Err(error) = watcher.watch(folder, Reach::Contents) {
                tracing::warn!("{error}");
            }
        }
        *self.inner.watcher.lock() = Some(watcher);

        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        handle.spawn(async move {
            while changes.recv().await.is_some() {
                tokio::time::sleep(RELOAD_DEBOUNCE).await;
                while changes.try_recv().is_ok() {}
                let Some(inner) = weak.upgrade() else {
                    break;
                };
                Hub { inner }.settle(BTreeSet::new()).await;
            }
        });
    }
}

impl Inner {
    fn mcp(&self) -> Arc<Mcp> {
        Arc::clone(&self.state().mcp)
    }

    fn state(&self) -> rikka_sync::MutexGuard<'_, HubState> {
        self.state.lock()
    }

    fn ground(&self) -> Ground<'_> {
        Ground {
            folder: self.folder.as_deref(),
            vault: self.vault.as_ref(),
        }
    }

    fn restock(&self) {
        let mcp = Arc::clone(&self.state().mcp);
        self.shelf.replace(mcp.kits());
    }

    fn beat(&self) {
        self.beats
            .send_modify(|beats| *beats = beats.wrapping_add(1));
    }
}

fn unanswered(session: &Session, reason: String) -> Error {
    Error::Unanswered {
        server: session.server().to_owned(),
        reason,
    }
}

fn origins(read: &[Catalog]) -> BTreeMap<String, usize> {
    let mut origins = BTreeMap::new();
    for (at, catalog) in read.iter().enumerate() {
        for server in catalog.servers() {
            origins.insert(server.name().to_owned(), at);
        }
    }
    origins
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hub")
            .field("places", &self.inner.places)
            .field("folder", &self.inner.folder)
            .finish_non_exhaustive()
    }
}
