use std::sync::Arc;
use std::time::Duration;

use browser_store::{ArchiveScope, ArchivedTab, ArchivedTabId, Store};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::internal::days;
use crate::internal::status::Status;
use crate::menus::space_name;
use crate::shell::Shell;
use crate::widgets::glyph_button;
use browser_model::SpaceId;

const LISTED: usize = 500;
const ROW: f32 = 48.0;
const WIDEST: f32 = 860.0;
const FETCH: &str = "rikka-browser-archive";
const SEARCH: &str = "rikka-browser-archive-search";
const SEARCH_PAUSE: Duration = Duration::from_millis(150);

#[derive(State)]
pub(crate) struct ArchivePage {
    #[untracked]
    store: Option<Store>,
    #[untracked]
    search: TextDocument,
    #[untracked]
    items: Tracked<Arc<Vec<Arc<ArchivedTab>>>>,
    status: Tracked<Status>,
}

impl ArchivePage {
    pub(crate) fn new() -> ArchivePage {
        ArchivePage {
            store: None,
            search: TextDocument::new(""),
            items: Tracked::new(Arc::new(Vec::new())),
            status: Tracked::new(Status::Waiting),
        }
    }

    pub(crate) fn open(&mut self, store: Option<Store>) {
        self.store = store;
        self.refresh();
    }

    pub(crate) fn has_store(&self) -> bool {
        self.store.is_some()
    }

    pub(crate) fn refresh(&mut self) {
        let Some(store) = self.store.clone() else {
            cancel(FETCH);
            self.status.set(Status::Ready);
            return;
        };
        let text = self.search.text();
        spawn_blocking_keyed(
            FETCH,
            move || {
                if let Err(error) = store.flush() {
                    tracing::warn!(%error, "failed to save browsing data");
                }
                store.search_archive(ArchiveScope::Everywhere, &text, LISTED)
            },
            move |page: &mut ArchivePage, found: browser_store::Result<Vec<ArchivedTab>>| {
                page.loaded(found);
            },
        );
    }

    fn loaded(&mut self, found: browser_store::Result<Vec<ArchivedTab>>) {
        match found {
            Ok(tabs) => {
                self.items
                    .set(Arc::new(tabs.into_iter().map(Arc::new).collect()));
                self.status.set(Status::Ready);
            }
            Err(error) => {
                tracing::warn!(%error, "failed to read archive");
                self.status.set(Status::Failed);
            }
        }
    }

    fn searched(&mut self) {
        after_keyed(SEARCH, SEARCH_PAUSE, |page: &mut ArchivePage| {
            page.refresh()
        });
    }

    pub(crate) fn drop_entry(&mut self, id: ArchivedTabId) {
        let items: Vec<Arc<ArchivedTab>> = self
            .items
            .iter()
            .filter(|entry| entry.id != id)
            .cloned()
            .collect();
        self.items.set(Arc::new(items));
    }

    fn remove(&mut self, id: ArchivedTabId) {
        if let Some(store) = &self.store {
            store.remove_archived(id);
        }
        self.drop_entry(id);
    }
}

impl std::fmt::Debug for ArchivePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArchivePage")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(shell: &Shell) -> Element {
    let page = &shell.pages().archive;
    if !page.has_store() {
        return status_page(MaterialSymbol::INVENTORY_2, tr!("browser.archive.private")).any();
    }
    let body = match (*page.status, page.items.is_empty()) {
        (Status::Failed, _) => {
            status_page(MaterialSymbol::WARNING, tr!("browser.archive.unreadable")).any()
        }
        (Status::Waiting, true) => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        (Status::Ready, true) => {
            status_page(MaterialSymbol::INVENTORY_2, tr!("browser.archive.empty")).any()
        }
        (_, false) => list(shell, page),
    };
    container(
        column! {
            text(tr!("browser.archive.title")).title();
            text_field(&page.search)
                .placeholder(tr!("browser.archive.search"))
                .on_change(|page: &mut ArchivePage| page.searched())
                .fill_width();
            body;
        }
        .gap(Space::NORMAL)
        .padding(Inset::all(Space::WIDE))
        .max_width(WIDEST)
        .fill(),
    )
    .align_x(Align::Center)
    .fill()
    .any()
}

fn list(shell: &Shell, page: &ArchivePage) -> Element {
    let items = Arc::clone(&page.items);
    let spaces: Vec<(SpaceId, String)> = shell
        .model()
        .spaces
        .list()
        .iter()
        .map(|space| (space.id, space_name(&space.name)))
        .collect();
    let count = items.len();
    lazy_column(count, move |slot| {
        let entry = &items[slot.index];
        let space = spaces
            .iter()
            .find(|(id, _)| *id == entry.space)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        archived(entry, space)
    })
    .uniform(ROW)
    .grow(1.0)
    .fill_width()
    .any()
}

fn archived(entry: &ArchivedTab, space: String) -> Element {
    let when = days::local_time(entry.archived_at)
        .map(|local_time| {
            format!(
                "{} {}",
                days::day(local_time.date, days::today()),
                local_time.time
            )
        })
        .unwrap_or_default();
    let title = crate::sidebar::titled(&entry.title, &entry.url);
    let host = crate::address::display_address(&entry.url).host;
    let id = entry.id;
    container(
        row! {
            column! {
                text(title).label().no_wrap().min_width(0.0);
                text(format!("{host} · {space}")).caption().muted().no_wrap().min_width(0.0);
            }
            .grow(1.0)
            .min_width(0.0);
            text(when).caption().muted().no_wrap();
            button(text(tr!("browser.archive.restore")))
                .secondary()
                .small()
                .focusless()
                .on_press(move |shell: &mut Shell| shell.restore_archived(id));
            glyph_button(MaterialSymbol::DELETE, 16.0)
                .on_press(move |page: &mut ArchivePage| page.remove(id));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .fill(),
    )
    .height(ROW)
    .fill_width()
    .any()
}
