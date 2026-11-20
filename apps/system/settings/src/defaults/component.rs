use rikka_app::prelude::*;
use rikka_mime::MimeType;
use rikka_xdg::{MimeApps, Watch};

use crate::defaults::catalogue::{Catalogue, Change, Kind, MimeInfo};
use crate::defaults::pane::Pane;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const READING: &str = "defaults-reading";
const WATCHING: &str = "defaults-watching";

#[derive(Default)]
pub(crate) struct Defaults {
    catalogue: Option<Result<Catalogue, String>>,
    pane: Pane,
    search: TextDocument,
    picked: Option<MimeType>,
    failure: Option<String>,
    watch: Option<Watch>,
    mime_info: MimeInfo,
}

impl Page for Defaults {
    fn opened(&mut self, _: &Pending, _: &Session) {
        self.read();
        self.watch();
    }

    fn closed(&mut self) {
        cancel(WATCHING);
        self.watch = None;
    }
}

impl Defaults {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn catalogue(&self) -> Option<&Result<Catalogue, String>> {
        self.catalogue.as_ref()
    }

    pub(crate) fn pane(&self) -> Pane {
        self.pane
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn picked(&self) -> Option<&Kind> {
        let picked = self.picked.as_ref()?;
        self.kinds()?.iter().find(|kind| kind.mime == *picked)
    }

    pub(crate) fn shown(&self) -> Vec<&Kind> {
        let query = self.search.text().trim().to_lowercase();
        self.kinds()
            .into_iter()
            .flatten()
            .filter(|kind| kind.matches(&query))
            .collect()
    }

    pub(crate) fn show(&mut self, pane: Pane) {
        if self.pane != pane {
            self.pane = pane;
            self.picked = None;
            self.search = TextDocument::default();
        }
    }

    pub(crate) fn pick(&mut self, index: usize) {
        self.picked = self.shown().get(index).map(|kind| kind.mime.clone());
    }

    pub(crate) fn unpick(&mut self) {
        self.picked = None;
    }

    pub(crate) fn choose_category(&mut self, group: usize, app: usize) {
        let Some(Ok(catalogue)) = &mut self.catalogue else {
            return;
        };
        let Some(chosen) = catalogue
            .groups
            .get(group)
            .and_then(|group| group.choice.offered.get(app))
            .map(|app| app.id.clone())
        else {
            return;
        };
        let Some(change) = catalogue.category_change(group, &chosen) else {
            return;
        };
        if let Some(group) = catalogue.groups.get_mut(group) {
            group.choice.current = Some(chosen);
            group.choice.chosen = true;
        }
        self.apply(change);
    }

    pub(crate) fn choose_terminal(&mut self, app: usize) {
        let Some(Ok(catalogue)) = &mut self.catalogue else {
            return;
        };
        let Some(chosen) = catalogue
            .terminal
            .offered
            .get(app)
            .map(|app| app.id.clone())
        else {
            return;
        };
        catalogue.terminal.current = Some(chosen.clone());
        catalogue.terminal.chosen = true;
        self.apply(Change::Terminal(chosen));
    }

    pub(crate) fn choose_picked(&mut self, app: usize) {
        let Some(mime) = self.picked.clone() else {
            return;
        };
        let pane = self.pane;
        let Some(Ok(catalogue)) = &mut self.catalogue else {
            return;
        };
        let Some(kind) = kinds_mut(catalogue, pane)
            .iter_mut()
            .find(|kind| kind.mime == mime)
        else {
            return;
        };
        let Some(chosen) = kind.choice.offered.get(app).map(|app| app.id.clone()) else {
            return;
        };
        kind.choice.current = Some(chosen.clone());
        kind.choice.chosen = true;
        self.apply(Change::Kind { app: chosen, mime });
    }

    pub(crate) fn forget_category(&mut self, group: usize) {
        let Some(Ok(catalogue)) = &self.catalogue else {
            return;
        };
        if let Some(group) = catalogue.groups.get(group) {
            self.apply(Change::Forget(group.types.clone()));
        }
    }

    pub(crate) fn forget_picked(&mut self) {
        if let Some(mime) = self.picked.clone() {
            self.apply(Change::Forget(vec![mime]));
        }
    }

    pub(crate) fn forget_terminal(&mut self) {
        self.apply(Change::ForgetTerminal);
    }

    pub(crate) fn forget_all(&mut self) {
        let Some(Ok(catalogue)) = &self.catalogue else {
            return;
        };
        self.apply(Change::ForgetAll(catalogue.chosen_types()));
    }

    fn apply(&mut self, change: Change) {
        self.failure = None;
        spawn_blocking(
            move || change.apply(),
            |defaults: &mut Defaults, applied: Result<(), String>| {
                defaults.failure = applied.err();
                defaults.read();
            },
        );
    }

    fn read(&mut self) {
        let mime_info = self.mime_info.clone();
        spawn_blocking_keyed(
            READING,
            move || Catalogue::read(&mime_info),
            |defaults: &mut Defaults, read: Result<Catalogue, String>| {
                defaults.catalogue = Some(read);
            },
        );
    }

    fn watch(&mut self) {
        if self.watch.is_some() {
            return;
        }
        let Some(apps) = MimeApps::session() else {
            return;
        };
        let (changed, changes) = futures_channel::mpsc::unbounded::<()>();
        match apps.watch(move || {
            let _ = changed.unbounded_send(());
        }) {
            Ok(watch) => self.watch = Some(watch),
            Err(error) => {
                tracing::warn!("failed to watch default applications: {error}");
                return;
            }
        }
        stream_keyed(WATCHING, changes, |defaults: &mut Defaults, ()| {
            defaults.read();
        });
    }

    fn kinds(&self) -> Option<&[Kind]> {
        let Some(Ok(catalogue)) = &self.catalogue else {
            return None;
        };
        match self.pane {
            Pane::Apps => None,
            Pane::Files => Some(&catalogue.files),
            Pane::Links => Some(&catalogue.links),
        }
    }
}

fn kinds_mut(catalogue: &mut Catalogue, pane: Pane) -> &mut [Kind] {
    match pane {
        Pane::Apps => &mut [],
        Pane::Files => &mut catalogue.files,
        Pane::Links => &mut catalogue.links,
    }
}

impl std::fmt::Debug for Defaults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Defaults")
            .field("pane", &self.pane)
            .finish_non_exhaustive()
    }
}
