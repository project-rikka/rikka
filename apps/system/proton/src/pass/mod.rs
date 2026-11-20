mod detail;
mod editor;
mod history;
mod list;
mod marks;
mod ops;
mod rows;
mod vaults;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use proton_session::ErrorBanner;
use proton_vaults::{Draft, Entry, ItemScope, ItemType, OpenedItem, SortKey, Vaults};
use rikka_app::prelude::*;
use rikka_proton_pass::{ItemId, ShareId};
use rikka_ui::Place;
use rikka_widgets::{column, row};

use crate::clients::Clients;
use crate::pane;
use crate::pass::detail::Deed;
use crate::pass::editor::Editor;
use crate::pass::history::History;
use crate::pass::vaults::VaultForm;
use crate::workspace::{ProductState, Workspace, WorkspaceId};

const TICK: Duration = Duration::from_secs(1);
const TICK_KEY: &str = "pass.tick";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Popup {
    New,
    Sort,
    Deeds,
}

#[derive(Debug, State)]
pub(crate) struct Pass {
    clients: Clients,
    workspace_id: WorkspaceId,
    vaults: Vaults,
    chosen: Option<(ShareId, ItemId)>,
    opened: Option<OpenedItem>,
    search: TextDocument,
    seeking: bool,
    revealed: HashSet<String>,
    now: jiff::Timestamp,
    loading: bool,
    started: bool,
    popup: Option<Popup>,
    telling: bool,
    error: ErrorBanner,
    place: Place,
    #[state]
    editor: Option<Editor>,
    #[state]
    form: Option<VaultForm>,
    #[state]
    history: Option<History>,
}

impl Pass {
    pub(crate) fn describe(&mut self, clients: Clients) {
        self.clients = clients;
    }

    pub(crate) fn new(clients: Clients, workspace_id: WorkspaceId) -> Pass {
        Pass {
            clients,
            workspace_id,
            vaults: Vaults::new(),
            chosen: None,
            opened: None,
            search: TextDocument::new(""),
            seeking: false,
            revealed: HashSet::new(),
            now: jiff::Timestamp::now(),
            loading: false,
            started: false,
            popup: None,
            telling: false,
            error: ErrorBanner::default(),
            place: Place::default(),
            editor: None,
            form: None,
            history: None,
        }
    }

    pub(crate) fn opened(&mut self) {
        self.every_keyed(TICK_KEY, TICK, |vault: &mut Pass| vault.tick());
        if self.started {
            return;
        }
        self.started = true;
        self.recall();
    }

    pub(crate) fn rest(&mut self) {
        cancel(TICK_KEY);
    }

    pub(crate) fn vaults(&self) -> &Vaults {
        &self.vaults
    }

    pub(crate) fn shown(&self) -> Option<&OpenedItem> {
        self.opened.as_ref()
    }

    pub(crate) fn chosen(&self) -> Option<&(ShareId, ItemId)> {
        self.chosen.as_ref()
    }

    pub(crate) fn editor(&self) -> Option<&Editor> {
        self.editor.as_ref()
    }

    pub(crate) fn form(&self) -> Option<&VaultForm> {
        self.form.as_ref()
    }

    pub(crate) fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }

    pub(crate) fn entry_of(&self, share: &ShareId, id: &ItemId) -> Option<&Entry> {
        self.vaults.entry(share, id)
    }

    pub(crate) fn popped(&mut self, popup: Option<Popup>) {
        self.popup = popup;
    }

    pub(crate) fn is_popped(&self, popup: Popup) -> bool {
        self.popup == Some(popup)
    }

    pub(crate) fn tells_more(&self) -> bool {
        self.telling
    }

    pub(crate) fn tell_more(&mut self) {
        self.telling = !self.telling;
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

    pub(crate) fn is_busy(&self) -> bool {
        self.loading
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn now(&self) -> jiff::Timestamp {
        self.now
    }

    pub(crate) fn is_revealed(&self, key: &str) -> bool {
        self.revealed.contains(key)
    }

    pub(crate) fn reveal(&mut self, key: String) {
        if !self.revealed.remove(&key) {
            self.revealed.insert(key);
        }
    }

    pub(crate) fn choose(&mut self, share: Option<ShareId>) {
        self.vaults.select(share);
        self.close();
    }

    pub(crate) fn show(&mut self, scope: ItemScope) {
        self.vaults.show(scope);
        self.close();
    }

    pub(crate) fn sort(&mut self, sorted: SortKey) {
        self.popup = None;
        self.vaults.sort(sorted);
    }

    pub(crate) fn look(&mut self) {
        let query = self.search.text();
        self.vaults.set_filter(query);
    }

    pub(crate) fn select(&mut self, at: usize) {
        let Some(entry) = self.vaults.shown().get(at).map(Arc::clone) else {
            return;
        };
        if self
            .chosen
            .as_ref()
            .is_some_and(|(share, id)| share == &entry.share && id == &entry.id)
        {
            skip_view();
            return;
        }

        self.close();
        self.chosen = Some((entry.share.clone(), entry.id.clone()));
        self.open(&entry.share, &entry.id);
    }

    pub(crate) fn write(&mut self, item_type: ItemType) {
        let Some(share) = self.where_to() else {
            return;
        };
        self.form = None;
        self.editor = Some(Editor::from_draft(Draft::fresh(share.clone(), item_type)));
        if item_type == ItemType::Alias {
            self.ask_choices(share);
        }
    }

    pub(crate) fn file_in(&mut self, at: usize) {
        let Some(share) = self
            .vaults
            .vaults()
            .get(at)
            .map(|vault| vault.share.clone())
        else {
            return;
        };
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        editor.move_to(share.clone());
        if editor.needs_choices() {
            self.ask_choices(share);
        }
    }

    pub(crate) fn edit(&mut self) {
        let Some(opened) = self.opened.as_ref() else {
            return;
        };
        self.editor = Some(Editor::from_draft(Draft::from_opened(opened)));
    }

    pub(crate) fn write_vault(&mut self) {
        self.form = Some(VaultForm::fresh());
        self.editor = None;
    }

    pub(crate) fn edit_vault(&mut self, share: &ShareId) {
        let Some(vault) = self.vaults.vault(share) else {
            return;
        };
        self.form = Some(VaultForm::from_vault(vault));
        self.editor = None;
    }

    pub(crate) fn close_vault_form(&mut self) {
        self.form = None;
    }

    pub(crate) fn close_editor(&mut self) {
        self.editor = None;
    }

    pub(crate) fn throw(&mut self) {
        let Some(entry) = self.entry() else {
            return;
        };
        let (share, id) = (entry.share.clone(), entry.id.clone());
        self.vaults
            .set_trashed(&share, std::slice::from_ref(&id), true);
        self.close();
        self.act(entry, false);
    }

    pub(crate) fn restore(&mut self) {
        let Some(entry) = self.entry() else {
            return;
        };
        let (share, id) = (entry.share.clone(), entry.id.clone());
        self.vaults
            .set_trashed(&share, std::slice::from_ref(&id), false);
        self.close();
        self.act(entry, true);
    }

    pub(crate) fn did(&mut self, deed: Deed) {
        self.popup = None;
        match deed {
            Deed::Pin => self.pin(),
            Deed::Watch => self.watch(),
            Deed::History => self.show_history(),
            Deed::Trash => self.throw(),
        }
    }

    pub(crate) fn close_history(&mut self) {
        self.history = None;
    }

    fn where_to(&self) -> Option<ShareId> {
        self.vaults.selected().cloned().or_else(|| {
            self.vaults
                .vaults()
                .first()
                .map(|vault| vault.share.clone())
        })
    }

    fn entry(&self) -> Option<Entry> {
        let (share, id) = self.chosen.as_ref()?;
        self.vaults.entry(share, id).cloned()
    }

    fn close(&mut self) {
        self.chosen = None;
        self.opened = None;
        self.editor = None;
        self.history = None;
        self.telling = false;
        self.revealed.clear();
    }

    fn tick(&mut self) {
        match self.opened.as_ref().is_some_and(OpenedItem::has_codes) {
            true => self.now = jiff::Timestamp::now(),
            false => skip_view(),
        }
    }
}

pub(crate) fn view(vault: &Pass) -> impl View {
    row! {
        list::vaults(vault);
        rule_upright();
        column! {
            list::toolbar(vault);
            rule();
            pane::error_banner(&vault.error, |vault: &mut Pass| &mut vault.error);
            list::view(vault).grow(1.0);
        }
        .width(pane::LISTING)
        .fill_height();
        rule_upright();
        body(vault).grow(1.0).fill_height();
        if vault.is_popped(Popup::New) {
            list::creations();
        }
        if vault.is_popped(Popup::Sort) {
            list::sorts(vault.vaults().sorted());
        }
        if vault.is_popped(Popup::Deeds)
            && let Some(opened) = vault.shown()
        {
            detail::deeds(vault, opened);
        }
    }
    .fill()
}

fn body(vault: &Pass) -> Element {
    if let Some(form) = vault.form() {
        return vaults::view(form);
    }
    if let Some(editor) = vault.editor() {
        return editor::view(editor, vault.vaults().vaults()).build();
    }
    if let Some(history) = vault.history() {
        return history::view(history).build();
    }
    match vault.shown() {
        Some(opened) => detail::view(vault, opened).build(),
        None => match vault.is_busy() && vault.chosen().is_some() {
            true => pane::working(tr!("proton.pass.opening")).build(),
            false => {
                pane::nothing(MaterialSymbol::SHIELD_LOCK, tr!("proton.pass.nothing-open")).build()
            }
        },
    }
}

impl ProductState for Pass {
    fn from_workspace(workspace: &mut Workspace) -> &mut Pass {
        &mut workspace.pass
    }

    fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
}
