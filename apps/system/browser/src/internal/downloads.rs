use browser_pages::ChromePage;
use rikka_app::prelude::*;
use rikka_web::DownloadItem;
use rikka_widgets::{column, row};

use crate::downloads::{item_row, newest_first};
use crate::shell::Shell;
use browser_model::{DownloadPhase, download_name};

const WIDEST: f32 = 860.0;
const ROW: f32 = 60.0;

#[derive(State)]
pub(crate) struct DownloadsPage {
    #[untracked]
    search: TextDocument,
    typed: Tracked<String>,
}

impl DownloadsPage {
    pub(crate) fn new() -> DownloadsPage {
        DownloadsPage {
            search: TextDocument::new(""),
            typed: Tracked::new(String::new()),
        }
    }

    fn searched(&mut self) {
        let typed = self.search.text().trim().to_lowercase();
        self.typed.set(typed);
    }
}

impl std::fmt::Debug for DownloadsPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DownloadsPage")
            .field("typed", &*self.typed)
            .finish_non_exhaustive()
    }
}

fn matches(item: &DownloadItem, typed: &str) -> bool {
    typed.is_empty()
        || download_name(item).to_lowercase().contains(typed)
        || item.url.to_lowercase().contains(typed)
}

pub(crate) fn view(shell: &Shell) -> Element {
    let page = &shell.pages().downloads;
    let profile = shell.model().profile_here(shell.id());
    let page_services = shell.browser().page_services();
    let downloads = page_services.downloads();
    let all = downloads.items(profile);
    let items: Vec<DownloadItem> = newest_first(all)
        .into_iter()
        .filter(|item| matches(item, &page.typed))
        .cloned()
        .collect();
    let any = all.iter().any(|item| DownloadPhase::of(item).ended());
    drop(downloads);
    let bar = row! {
        text_field(&page.search)
            .placeholder(tr!("browser.downloads.search"))
            .on_change(|page: &mut DownloadsPage| page.searched())
            .grow(1.0)
            .min_width(0.0);
        button(text(tr!("browser.downloads.clear")))
            .secondary()
            .small()
            .enabled(any)
            .on_press(|shell: &mut Shell| shell.clear_downloads());
        button(text(tr!("browser.downloads.location")))
            .ghost()
            .small()
            .on_press(|shell: &mut Shell| {
                shell.open_page(ChromePage::Downloads.address().to_owned())
            });
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width();
    let body = match items.is_empty() {
        true => status_page(MaterialSymbol::DOWNLOAD, tr!("browser.downloads.empty")).any(),
        false => {
            let count = items.len();
            lazy_column(count, move |slot| {
                let item = &items[slot.index];
                container(item_row(item))
                    .height(ROW)
                    .fill_width()
                    .build()
                    .keyed(item.id)
            })
            .grow(1.0)
            .fill_width()
            .any()
        }
    };
    container(
        column! {
            text(tr!("browser.downloads.title")).title();
            bar;
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
