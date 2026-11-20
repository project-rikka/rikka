mod form;
mod list;

use std::time::Duration;

use proton_codes::{CodeStore, Setup, StoredCode, recall, sync};
use proton_session::{ErrorBanner, Failure};
use rikka_app::prelude::*;
use rikka_proton_api::offload;
use rikka_proton_authenticator::SyncOutcome;
use rikka_proton_cache::{Cache, Slot, Table};
use rikka_ui::Place;
use rikka_widgets::{column, row};

use crate::clients::Clients;
use crate::codes::form::Form;
use crate::pane;
use crate::workspace::{ProductState, Workspace, WorkspaceId};

const TICK: Duration = Duration::from_secs(1);
const TICK_KEY: &str = "codes.tick";
const PENDING: Table<Vec<StoredCode>> = Table::new("app.codes.pending");

#[derive(Debug, State)]
pub(crate) struct Codes {
    clients: Clients,
    pending: Slot<Vec<StoredCode>>,
    workspace_id: WorkspaceId,
    store: CodeStore,
    search: TextDocument,
    seeking: bool,
    now: jiff::Timestamp,
    syncing: bool,
    recalling: bool,
    waiting_sync: bool,
    started: bool,
    error: ErrorBanner,
    place: Place,
    #[state]
    form: Option<Form>,
}

impl Codes {
    pub(crate) fn describe(&mut self, clients: Clients) {
        self.clients = clients;
    }

    pub(crate) fn new(clients: Clients, cache: Cache, workspace_id: WorkspaceId) -> Codes {
        let codes = Codes {
            clients,
            pending: cache.slot(&PENDING, ""),
            workspace_id,
            store: CodeStore::new(),
            search: TextDocument::new(""),
            seeking: false,
            now: jiff::Timestamp::now(),
            syncing: false,
            recalling: true,
            waiting_sync: false,
            started: false,
            error: ErrorBanner::default(),
            place: Place::default(),
            form: None,
        };
        codes.recall();
        codes
    }

    pub(crate) fn opened(&mut self) {
        self.every_keyed(TICK_KEY, TICK, |codes: &mut Codes| codes.tick());
        if self.started {
            return;
        }
        self.started = true;
        self.sync();
    }

    pub(crate) fn rest(&mut self) {
        cancel(TICK_KEY);
    }

    pub(crate) fn store(&self) -> &CodeStore {
        &self.store
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn seeking(&self) -> bool {
        self.seeking
    }

    pub(crate) fn seek(&mut self) {
        self.search.select_all();
        self.seeking = true;
    }

    pub(crate) fn sought(&mut self) {
        if !self.seeking {
            skip_view();
            return;
        }
        self.seeking = false;
    }

    pub(crate) fn now(&self) -> jiff::Timestamp {
        self.now
    }

    pub(crate) fn is_syncing(&self) -> bool {
        self.syncing
    }

    pub(crate) fn is_recalling(&self) -> bool {
        self.recalling
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn form(&self) -> Option<&Form> {
        self.form.as_ref()
    }

    pub(crate) fn look(&mut self) {
        let query = self.search.text();
        self.store.set_filter(query);
    }

    pub(crate) fn write(&mut self) {
        self.form = Some(Form::new_id());
    }

    pub(crate) fn edit(&mut self, id: &str) {
        let Some(cached) = self.store.get(id) else {
            return;
        };
        self.form = Some(Form::from_code(cached));
    }

    pub(crate) fn close_form(&mut self) {
        self.form = None;
    }

    pub(crate) fn keep(&mut self) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let otp = match form.setup().otp() {
            Ok(otp) => otp,
            Err(error) => {
                form.failed(&error);
                return;
            }
        };
        let note = form.note();
        let at = jiff::Timestamp::now();

        let cached = match form.id() {
            Some(id) => self.store.change(id, &otp, note, at),
            None => {
                self.store.add(&otp, note, at);
                Ok(())
            }
        };
        match cached {
            Ok(()) => self.settle_form(),
            Err(error) => form.failed(&error),
        }
    }

    pub(crate) fn paste(&mut self) {
        let (Some(pasted), Some(form)) = (text_of(Selection::Clipboard), self.form.as_mut()) else {
            return;
        };
        let pasted = pasted.trim();

        if let Some(setup) = Setup::read(pasted) {
            form.fill(setup);
            return;
        }
        if !pasted.contains("://") {
            form.took_secret(pasted);
            return;
        }
        let at = jiff::Timestamp::now();
        match self.store.import(pasted, form.note(), at) {
            Ok(_) => self.settle_form(),
            Err(error) => form.failed(&error),
        }
    }

    fn settle_form(&mut self) {
        self.form = None;
        self.keep_kept();
        self.sync();
    }

    pub(crate) fn drop_code(&mut self, id: &str) {
        self.store.remove(id);
        self.keep_kept();
        self.sync();
    }

    pub(crate) fn copy(&mut self, id: &str) {
        let Some(code) = self
            .store
            .get(id)
            .and_then(|cached| cached.code_at(self.now))
        else {
            return;
        };
        set_text_of(Selection::Clipboard, &code.now);
    }

    pub(crate) fn sync(&mut self) {
        if self.recalling {
            self.waiting_sync = true;
            return;
        }
        let clients = self.clients.clone();
        let store = self.store.all().to_vec();
        self.syncing = true;

        self.spawn_keyed(
            "codes.sync",
            async move {
                let client = clients.codes()?;
                Ok(sync(&client, &store).await?)
            },
            |codes: &mut Codes, outcome: Result<SyncOutcome, Failure>| {
                codes.syncing = false;
                if let Some(outcome) = codes.error.capture(outcome) {
                    codes.store.apply_sync(&outcome);
                    codes.keep_kept();
                }
            },
        );
    }

    fn keep_kept(&self) {
        if self.recalling {
            return;
        }
        let staged = self.pending.stage(self.store.pending());
        self.spawn_blocking(move || staged.write(), |_: &mut Codes, _| skip_view());
    }

    fn recall(&self) {
        let clients = self.clients.clone();
        let pending = self.pending.clone();
        self.spawn_keyed(
            "codes.recall",
            async move {
                let pending = offload(move || pending.read().unwrap_or_default()).await;
                let synced: Result<Vec<StoredCode>, Failure> = async {
                    let client = clients.codes()?;
                    Ok(recall(&client).await?)
                }
                .await;
                Recalled { synced, pending }
            },
            |codes: &mut Codes, recalled: Recalled| codes.recalled(recalled),
        );
    }

    fn recalled(&mut self, recalled: Recalled) {
        self.recalling = false;
        let edited = !self.store.all().is_empty();
        let synced = self.error.capture(recalled.synced).unwrap_or_default();
        self.store.recall(synced, recalled.pending);
        if edited {
            self.keep_kept();
        }
        if std::mem::take(&mut self.waiting_sync) {
            self.sync();
        }
    }

    fn tick(&mut self) {
        match self.store.filtered().is_empty() {
            true => skip_view(),
            false => self.now = jiff::Timestamp::now(),
        }
    }
}

struct Recalled {
    synced: Result<Vec<StoredCode>, Failure>,
    pending: Vec<StoredCode>,
}

pub(crate) fn view(codes: &Codes) -> impl View {
    column! {
        list::toolbar(codes);
        rule();
        pane::error_banner(&codes.error, |codes: &mut Codes| &mut codes.error);
        row! {
            list::view(codes).grow(1.0);
            if let Some(form) = codes.form() {
                rule_upright();
                form::view(form).width(pane::SIDEBAR + 160.0).fill_height();
            }
        }
        .grow(1.0);
    }
    .fill()
}

impl ProductState for Codes {
    fn from_workspace(workspace: &mut Workspace) -> &mut Codes {
        &mut workspace.codes
    }

    fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
}
