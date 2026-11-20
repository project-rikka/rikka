use std::path::PathBuf;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_widgets::{column, row};

use crate::downloads::item_row;
use crate::shell::Shell;
use browser_model::DownloadItem;

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

pub(crate) fn matches(item: &DownloadItem, typed: &str) -> bool {
    typed.is_empty()
        || item.name.to_lowercase().contains(typed)
        || item.url.to_lowercase().contains(typed)
}

pub(crate) fn view(shell: &Shell) -> Element {
    let page = &shell.pages().downloads;
    let profile = shell.model().profile_here(shell.id());
    let page_services = shell.browser().page_services();
    let downloads = page_services.downloads();
    let items: Vec<DownloadItem> = downloads
        .items(profile)
        .filter(|item| matches(item, &page.typed))
        .cloned()
        .collect();
    let any = downloads.items(profile).any(|item| item.state.ended());
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
                    .keyed(item.key)
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

pub(crate) fn settings(shell: &Shell) -> Element {
    let preferences = shell.browser().preferences();
    let folder = preferences.download_folder();
    let asks = preferences.download_asks();
    page([section(
        tr!("browser.downloads.settings"),
        column! {
            field(
                tr!("browser.downloads.folder"),
                row! {
                    text(folder.display().to_string())
                        .caption()
                        .muted()
                        .no_wrap()
                        .shrink(1.0)
                        .min_width(0.0);
                    button(text(tr!("browser.downloads.change-folder")))
                        .secondary()
                        .small()
                        .on_press(|shell: &mut Shell| shell.choose_download_folder());
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            );
            field(
                tr!("browser.downloads.ask"),
                switch(asks).on_toggle(|shell: &mut Shell, on: bool| {
                    shell.browser().preferences().set_download_asks(on);
                }),
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}

impl Shell {
    fn choose_download_folder(&mut self) {
        let current = self.browser().preferences().download_folder();
        let chooser = Chooser::open(tr!("browser.downloads.folder").to_string())
            .parent(window_identifier())
            .directory(true)
            .current_folder(current);
        spawn_blocking(
            move || chooser.run(),
            |shell: &mut Shell, chosen: rikka_dialog::Result<ChooserResponse>| {
                let folder: Option<PathBuf> = match chosen {
                    Ok(ChooserResponse::Files(files)) => files.into_iter().next(),
                    Ok(ChooserResponse::Cancelled) => None,
                    Err(error) => {
                        tracing::warn!(%error, "failed to choose download folder");
                        None
                    }
                };
                if let Some(folder) = folder {
                    shell.browser().preferences().set_download_folder(&folder);
                }
            },
        );
    }
}
