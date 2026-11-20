mod compose;
mod folders;
mod list;
mod reading;

use std::sync::Arc;
use std::time::Duration;

use proton_config::Config;
use proton_mailbox::{
    Ask, Attaching, Draft, Folder, Invalidation, Link, Listing, Mailbox, MessageAttachment,
    OpenedMessage, OpenedThread, Row, RowId, ThreadCache, attachment, cached_folders, cached_page,
    delete, folders, mark_read, move_rows, page, prefetch, prefetch_candidates, read, set_label,
    trash,
};
use proton_session::{ErrorBanner, Failure};
use rikka_app::prelude::*;
use rikka_proton_api::offload;
use rikka_proton_mail::{Event, LabelId, MailClient};
use rikka_ui::Place;
use rikka_widgets::{column, row};

use crate::clients::Clients;
use crate::mail::compose::Compose;
use crate::pane;
use crate::workspace::{ProductState, Workspace, WorkspaceId};

const AHEAD: &str = "mail.ahead";
const FOLDERS: &str = "mail.folders";
const LISTING: &str = "mail.listing";
const READING: &str = "mail.read";
const TYPING: &str = "mail.typing";

const SETTLED: Duration = Duration::from_millis(350);

#[derive(Debug, State)]
pub(crate) struct Mail {
    clients: Clients,
    workspace_id: WorkspaceId,
    mailbox: Mailbox,
    grouped: bool,
    unread_only: bool,
    search: TextDocument,
    seeking: bool,
    selected: Option<RowId>,
    reading: Option<Arc<OpenedThread>>,
    readings: ThreadCache,
    ahead_halted: bool,
    loading: bool,
    opening: bool,
    error: ErrorBanner,
    started: bool,
    place: Place,
    #[state]
    compose: Option<Compose>,
}

impl Mail {
    pub(crate) fn describe(&mut self, clients: Clients) {
        self.clients = clients;
    }

    pub(crate) fn new(clients: Clients, workspace_id: WorkspaceId, config: &Config) -> Mail {
        Mail {
            clients,
            workspace_id,
            mailbox: Mailbox::new(),
            grouped: config.conversations(),
            unread_only: false,
            search: TextDocument::new(""),
            seeking: false,
            selected: None,
            reading: None,
            readings: ThreadCache::new(),
            ahead_halted: false,
            loading: false,
            opening: false,
            error: ErrorBanner::default(),
            started: false,
            place: Place::default(),
            compose: None,
        }
    }

    pub(crate) fn opened(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.recall();
        self.refresh();
    }

    pub(crate) fn group(&mut self, grouped: bool) {
        if self.grouped == grouped {
            return;
        }
        self.grouped = grouped;
        self.mailbox.clear_folder_cache();
        self.reading = None;
        self.selected = None;
        self.refresh();
    }

    pub(crate) fn mailbox(&self) -> &Mailbox {
        &self.mailbox
    }

    pub(crate) fn reading(&self) -> Option<&OpenedThread> {
        self.reading.as_deref()
    }

    pub(crate) fn refresh(&mut self) {
        self.read_folders();
        self.read_page(0);
    }

    pub(crate) fn show(&mut self, folder: LabelId) {
        if self.mailbox.shown() == &folder {
            skip_view();
            return;
        }
        self.mailbox.show(folder);
        self.selected = None;
        self.reading = None;
        if self.mailbox.is_empty() {
            self.recall();
        }
        self.read_page(0);
    }

    pub(crate) fn look(&mut self) {
        let query = self.search.text();
        self.after_keyed(TYPING, SETTLED, move |mail: &mut Mail| {
            if mail.search.text() == query {
                mail.read_page(0);
            }
        });
    }

    pub(crate) fn only_unread(&mut self, unread: bool) {
        self.unread_only = unread;
        self.read_page(0);
    }

    pub(crate) fn more(&mut self) {
        if self.loading || !self.mailbox.has_more() {
            return;
        }
        self.read_page(self.mailbox.page() + 1);
    }

    pub(crate) fn select(&mut self, at: usize) {
        let Some(row) = self.mailbox.row(at).cloned() else {
            return;
        };
        if self.selected.as_ref() == Some(&row.id) {
            skip_view();
            return;
        }
        self.selected = Some(row.id.clone());
        self.compose = None;
        if row.unread {
            self.read_now(&row.id);
        }
        match self.readings.get(&row.id) {
            Some(reading) => {
                self.opening = false;
                self.reading = Some(reading);
            }
            None => {
                self.reading = None;
                self.open(row);
            }
        }
        self.prefetch_nearby();
    }

    pub(crate) fn shown(&self) -> Option<&Row> {
        let id = self.selected.as_ref()?;
        self.mailbox.rows().iter().find(|row| &row.id == id)
    }

    pub(crate) fn compose(&self) -> Option<&Compose> {
        self.compose.as_ref()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.loading
    }

    pub(crate) fn is_opening(&self) -> bool {
        self.opening
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.mailbox.place(self.selected.as_ref()?)
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn is_unread_only(&self) -> bool {
        self.unread_only
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

    pub(crate) fn write(&mut self) {
        self.compose = Some(Compose::fresh(self.clients.session(), self.workspace_id));
    }

    pub(crate) fn answer(&mut self, everyone: bool) {
        let Some(opened) = self.newest() else {
            return;
        };
        let header = reply_header(&opened);
        self.compose = Some(Compose::from_draft(
            Draft::replying(&opened, everyone, None, &header),
            self.clients.session(),
            self.workspace_id,
        ));
    }

    pub(crate) fn pass_on(&mut self) {
        let Some(opened) = self.newest() else {
            return;
        };
        let header = reply_header(&opened);
        self.compose = Some(Compose::from_draft(
            Draft::forwarding(&opened, None, &header),
            self.clients.session(),
            self.workspace_id,
        ));
    }

    pub(crate) fn follow(&mut self, target: &str) {
        match Link::parse(target) {
            Some(Link::Web(url)) => {
                if let Err(error) = rikka_open::open(url.as_str()) {
                    self.error.set(error);
                }
            }
            Some(Link::Compose(draft)) => {
                self.compose = Some(Compose::from_draft(
                    draft,
                    self.clients.session(),
                    self.workspace_id,
                ));
            }
            None => self
                .error
                .set(Failure::new(tr!("proton.mail.unopenable-link").as_str())),
        }
    }

    pub(crate) fn edit_draft(&mut self) {
        let Some(opened) = self.newest() else {
            return;
        };
        self.compose = Some(Compose::from_draft(
            Draft::editing(&opened),
            self.clients.session(),
            self.workspace_id,
        ));
    }

    pub(crate) fn close_compose(&mut self) {
        self.compose = None;
    }

    pub(crate) fn discard_draft(&mut self) {
        let Some(id) = self.compose.as_ref().and_then(Compose::draft_id) else {
            self.compose = None;
            return;
        };

        self.compose = None;
        let client = self.clients.mail();
        self.spawn(
            async move { Ok(proton_mailbox::discard_draft(&client, &id).await?) },
            |mail: &mut Mail, done: Result<(), Failure>| match done {
                Ok(()) => mail.refresh(),
                Err(reason) => mail.error.set(reason),
            },
        );
    }

    pub(crate) fn send_draft(&mut self) {
        self.give(true);
    }

    pub(crate) fn keep_draft(&mut self) {
        self.give(false);
    }

    pub(crate) fn attached(&mut self, files: Vec<Attaching>) {
        match self.compose.as_mut() {
            Some(compose) => compose.took(files),
            None => skip_view(),
        }
    }

    pub(crate) fn compose_failed(&mut self, reason: Failure) {
        if let Some(compose) = self.compose.as_mut() {
            compose.failed(reason);
        }
    }

    fn give(&mut self, now: bool) {
        let Some(compose) = self.compose.as_mut() else {
            return;
        };
        let draft = compose.draft();
        if now && !draft.is_sendable() {
            compose.failed(Failure::new(tr!("proton.mail.no-recipient").as_str()));
            return;
        }

        compose.working();
        let client = self.clients.mail();
        self.spawn(
            async move {
                match now {
                    true => proton_mailbox::send(&client, &draft).await.map(drop),
                    false => proton_mailbox::save_draft(&client, &draft).await.map(drop),
                }
                .map_err(Failure::from)
            },
            |mail: &mut Mail, done: Result<(), Failure>| match done {
                Ok(()) => mail.sent(),
                Err(reason) => mail.compose_failed(reason),
            },
        );
    }

    pub(crate) fn sent(&mut self) {
        self.compose = None;
        self.reading = None;
        self.selected = None;
        self.refresh();
    }

    pub(crate) fn star(&mut self, starred: bool) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let ids = vec![row.id.clone()];
        self.mailbox.set_starred(&ids, starred);
        self.act(
            move |client| async move { proton_mailbox::set_starred(&client, &ids, starred).await },
        );
    }

    pub(crate) fn mark(&mut self, read: bool) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let ids = vec![row.id.clone()];
        let folder = self.mailbox.shown().clone();
        self.mailbox.set_read(&ids, read);
        if !read {
            self.reading = None;
            self.selected = None;
        }
        self.act(move |client| async move { mark_read(&client, &ids, read, &folder).await });
    }

    pub(crate) fn move_to(&mut self, label: LabelId) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let ids = vec![row.id.clone()];
        let from = self.mailbox.shown().clone();
        self.leave(&ids);
        self.act(move |client| async move { move_rows(&client, &ids, &from, &label).await });
    }

    pub(crate) fn throw(&mut self) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let ids = vec![row.id.clone()];
        let from = self.mailbox.shown().clone();
        self.leave(&ids);
        self.act(move |client| async move { trash(&client, &ids, &from).await });
    }

    pub(crate) fn erase(&mut self) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let ids = vec![row.id.clone()];
        let folder = self.mailbox.shown().clone();
        self.leave(&ids);
        self.act(move |client| async move { delete(&client, &ids, &folder).await });
    }

    pub(crate) fn toggle_label(&mut self, label: LabelId) {
        let Some(row) = self.shown().cloned() else {
            return;
        };
        let on = !row.has_label(&label);
        let ids = vec![row.id.clone()];
        self.mailbox.set_label(&ids, &label, on);
        self.act(move |client| async move { set_label(&client, &ids, &label, on).await });
    }

    pub(crate) fn save_file(&mut self, file: MessageAttachment) {
        let client = self.clients.mail();
        self.spawn(
            async move {
                let bytes = attachment(&client, &file).await?;
                offload(move || proton_files::save_download(&file.name, &bytes))
                    .await
                    .map_err(Failure::from)
            },
            |mail: &mut Mail, saved: Result<std::path::PathBuf, Failure>| match saved.err() {
                Some(reason) => mail.error.set(reason),
                None => skip_view(),
            },
        );
    }

    fn newest(&self) -> Option<OpenedMessage> {
        self.reading.as_ref()?.newest().cloned()
    }

    fn leave(&mut self, ids: &[RowId]) {
        self.mailbox.remove(ids);
        self.reading = None;
        self.selected = None;
    }

    fn read_now(&mut self, id: &RowId) {
        let ids = vec![id.clone()];
        let folder = self.mailbox.shown().clone();
        self.mailbox.set_read(&ids, true);
        self.act(move |client| async move { mark_read(&client, &ids, true, &folder).await });
    }

    fn open(&mut self, row: Row) {
        self.opening = true;
        let id = row.id.clone();
        self.spawn_keyed(
            READING,
            self.reading_of(row),
            move |mail: &mut Mail, opened: Result<Arc<OpenedThread>, Failure>| {
                if mail.selected.as_ref() != Some(&id) {
                    skip_view();
                    return;
                }
                mail.opening = false;
                match opened {
                    Ok(reading) => mail.reading = Some(reading),
                    Err(reason) => mail.error.set(reason),
                }
            },
        );
    }

    fn reread(&mut self) {
        let Some(id) = self.reading.as_ref().map(|reading| reading.id.clone()) else {
            return;
        };
        if self.readings.contains(&id) {
            return;
        }
        let Some(row) = self.mailbox.rows().iter().find(|row| row.id == id).cloned() else {
            return;
        };
        self.spawn_keyed(
            READING,
            self.reading_of(row),
            move |mail: &mut Mail, opened: Result<Arc<OpenedThread>, Failure>| match opened {
                Ok(reading) if mail.selected.as_ref() == Some(&id) => mail.reading = Some(reading),
                _ => skip_view(),
            },
        );
    }

    fn reading_of(
        &self,
        row: Row,
    ) -> impl Future<Output = Result<Arc<OpenedThread>, Failure>> + Send + 'static {
        let client = self.clients.mail();
        let readings = self.readings.clone();
        async move {
            let id = row.id.clone();
            Ok(readings
                .read(&id, async move { read(&client, &row).await })
                .await?)
        }
    }

    fn prefetch_nearby(&mut self) {
        if self.ahead_halted {
            return;
        }
        let candidates = prefetch_candidates(self.mailbox.rows(), self.selected(), &self.readings);
        if candidates.is_empty() {
            return;
        }
        let client = self.clients.mail();
        let readings = self.readings.clone();
        self.spawn_keyed(
            AHEAD,
            async move { prefetch(&client, &readings, candidates).await.is_ok() },
            |mail: &mut Mail, finished: bool| {
                mail.ahead_halted = !finished;
                skip_view();
            },
        );
    }

    fn read_folders(&mut self) {
        let grouped = self.grouped;
        let client = self.clients.mail();
        self.spawn_keyed(
            FOLDERS,
            async move { Ok(folders(&client, grouped).await?) },
            |mail: &mut Mail, result: Result<Vec<Folder>, Failure>| match result {
                Ok(folders) => {
                    mail.mailbox.set_folders(folders);
                }
                Err(reason) => mail.error.set(reason),
            },
        );
    }

    fn read_page(&mut self, page: u32) {
        let ask = self.ask(page);
        let client = self.clients.mail();
        self.loading = true;

        self.spawn_keyed(
            LISTING,
            async move {
                let listing = page_of(&client, &ask).await;
                (ask, listing)
            },
            |mail: &mut Mail, (ask, listing): (Ask, Result<Listing, Failure>)| {
                mail.loading = false;
                match listing {
                    Ok(listing) => {
                        mail.mailbox.apply_listing(listing, &ask);
                        if ask.is_first() {
                            mail.ahead_halted = false;
                            mail.prefetch_nearby();
                        }
                    }
                    Err(reason) => mail.error.set(reason),
                }
            },
        );
    }

    fn ask(&self, page: u32) -> Ask {
        let kind = self.mailbox.kind();
        Ask::new(self.mailbox.shown().clone(), kind, self.grouped)
            .looking_for(self.search.text())
            .unread_only(self.unread_only)
            .at_page(page)
    }

    pub(crate) fn apply_event(&mut self, event: &Event) {
        if !self.started {
            skip_view();
            return;
        }
        self.readings.apply_event(event);
        self.reread();
        let invalidation = self.mailbox.apply_event(event, self.grouped);
        if !invalidation.any() {
            skip_view();
            return;
        }
        let Invalidation {
            folders, listing, ..
        } = invalidation;
        if folders {
            self.read_folders();
        }
        if listing {
            self.read_page(0);
        }
    }

    fn recall(&mut self) {
        let wants_folders = self.mailbox.folders().is_empty();
        let grouped = self.grouped;
        let ask = Ask::new(self.mailbox.shown().clone(), self.mailbox.kind(), grouped);
        let client = self.clients.mail();
        self.spawn_blocking(
            move || {
                let folders = wants_folders
                    .then(|| cached_folders(&client, grouped))
                    .flatten();
                let listing = cached_page(&client, &ask);
                (ask.folder, folders, listing)
            },
            |mail: &mut Mail, (folder, folders, listing): Recalled| {
                if let Some(folders) = folders
                    && mail.mailbox.folders().is_empty()
                {
                    mail.mailbox.set_folders(folders);
                }
                if let Some(listing) = listing {
                    mail.mailbox.load_cached(&folder, listing.rows);
                }
            },
        );
    }

    fn act<F, Fut>(&mut self, work: F)
    where
        F: FnOnce(MailClient) -> Fut + Send + 'static,
        Fut: Future<Output = proton_mailbox::Result<()>> + Send,
    {
        let client = self.clients.mail();
        self.spawn(
            async move { work(client).await.map_err(Failure::from) },
            |mail: &mut Mail, done: Result<(), Failure>| match done {
                Ok(()) => mail.refresh(),
                Err(reason) => mail.error.set(reason),
            },
        );
    }
}

type Recalled = (LabelId, Option<Vec<Folder>>, Option<Listing>);

async fn page_of(client: &MailClient, ask: &Ask) -> Result<Listing, Failure> {
    Ok(page(client, ask).await?)
}

fn reply_header(opened: &OpenedMessage) -> String {
    tr!(
        "proton.mail.quote",
        who = opened.sender.shown(),
        at = &crate::wording::whole(opened.time),
    )
    .as_str()
    .to_owned()
}

pub(crate) fn view(mail: &Mail) -> impl View {
    row! {
        folders::view(mail);
        rule_upright();
        column! {
            list::toolbar(mail);
            rule();
            pane::error_banner(&mail.error, |mail: &mut Mail| &mut mail.error);
            list::view(mail).grow(1.0);
        }
        .width(pane::LISTING)
        .fill_height();
        rule_upright();
        body(mail).grow(1.0).fill_height();
    }
    .fill()
}

fn body(mail: &Mail) -> Element {
    if let Some(compose) = mail.compose() {
        return compose::view(compose).build();
    }
    if mail.is_opening() {
        return pane::working(tr!("proton.mail.opening")).build();
    }
    match mail.reading() {
        Some(reading) => reading::view(mail, reading).build(),
        None => pane::nothing(MaterialSymbol::MAIL, tr!("proton.mail.nothing-open")).build(),
    }
}

impl ProductState for Mail {
    fn from_workspace(workspace: &mut Workspace) -> &mut Mail {
        &mut workspace.mail
    }

    fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
}
