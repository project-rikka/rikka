use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_web::DownloadItem;
use rikka_widgets::{column, row};

use crate::command::Command;
use crate::shell::Shell;
use crate::widgets::glyph_button;
use browser_model::{DownloadPhase, download_name};

const WIDTH: f32 = 360.0;
const SHOWN: usize = 6;
const GLYPH: f32 = 16.0;

pub(crate) fn panel(shell: &Shell) -> Element {
    let profile = shell.model().profile_here(shell.id());
    let page_services = shell.browser().page_services();
    let downloads = page_services.downloads();
    let rows: Vec<Element> = newest_first(downloads.items(profile))
        .into_iter()
        .take(SHOWN)
        .map(|item| row(item).build().keyed(item.id))
        .collect();
    let body = match rows.is_empty() {
        true => text(tr!("browser.downloads.empty")).muted().any(),
        false => column_of(rows).gap(Space::HAIR).fill_width().any(),
    };
    column! {
        row! {
            text(tr!("browser.downloads.title")).label().semibold().grow(1.0);
            button(text(tr!("browser.downloads.show-all")))
                .ghost()
                .small()
                .focusless()
                .on_press(|shell: &mut Shell| {
                    shell.overlays.close_popup();
                    shell.command(Command::Downloads);
                });
        }
        .align_items(Align::Center)
        .fill_width();
        body;
    }
    .gap(Space::SNUG)
    .width(WIDTH)
    .any()
}

pub(crate) fn newest_first(items: &[DownloadItem]) -> Vec<&DownloadItem> {
    let mut sorted: Vec<&DownloadItem> = items.iter().collect();
    sorted.sort_by_key(|item| std::cmp::Reverse(item.id));
    sorted
}

fn bytes(value: f64) -> u64 {
    value.max(0.0) as u64
}

fn moved(item: &DownloadItem) -> String {
    let received = format_bytes(bytes(item.bytes_received));
    match item.total_bytes > 0.0 {
        true => tr!(
            "browser.downloads.progress",
            received = &received,
            total = &format_bytes(bytes(item.total_bytes))
        )
        .to_string(),
        false => received,
    }
}

pub(crate) fn status(item: &DownloadItem) -> String {
    match DownloadPhase::of(item) {
        DownloadPhase::Running => moved(item),
        DownloadPhase::Paused => {
            tr!("browser.downloads.paused", progress = &moved(item)).to_string()
        }
        DownloadPhase::Finished if !item.exists => tr!("browser.downloads.deleted").to_string(),
        DownloadPhase::Finished => format_bytes(bytes(item.total_bytes.max(item.bytes_received))),
        DownloadPhase::Failed => tr!("browser.downloads.failed").to_string(),
        DownloadPhase::Cancelled => tr!("browser.downloads.cancelled").to_string(),
    }
}

pub(crate) fn row(item: &DownloadItem) -> impl View {
    let id = item.id;
    let phase = DownloadPhase::of(item);
    let symbol = match phase {
        DownloadPhase::Failed | DownloadPhase::Cancelled => MaterialSymbol::ERROR,
        _ => MaterialSymbol::DESCRIPTION,
    };
    let tool = |symbol: MaterialSymbol, act: fn(&mut Shell, rikka_web::DownloadId)| {
        glyph_button(symbol, GLYPH)
            .on_press(move |shell: &mut Shell| act(shell, id))
            .any()
    };
    let forget = tool(MaterialSymbol::CLOSE, Shell::forget_download);
    let cancel = tool(MaterialSymbol::CLOSE, Shell::cancel_download);
    let actions: Vec<Element> = match phase {
        DownloadPhase::Running => vec![tool(MaterialSymbol::PAUSE, Shell::pause_download), cancel],
        DownloadPhase::Paused if item.can_resume => {
            vec![
                tool(MaterialSymbol::PLAY_ARROW, Shell::resume_download),
                cancel,
            ]
        }
        DownloadPhase::Paused => vec![cancel],
        DownloadPhase::Finished if item.exists => vec![
            tool(MaterialSymbol::FOLDER_OPEN, Shell::show_download_folder),
            tool(MaterialSymbol::DELETE, Shell::delete_download_file),
            forget,
        ],
        DownloadPhase::Finished => vec![forget],
        DownloadPhase::Failed | DownloadPhase::Cancelled if item.can_resume => {
            vec![
                tool(MaterialSymbol::REFRESH, Shell::resume_download),
                forget,
            ]
        }
        DownloadPhase::Failed | DownloadPhase::Cancelled => vec![forget],
    };
    let progress = (!phase.ended()).then(|| match item.progress() {
        Some(fraction) => progress(fraction as f32).fill_width().any(),
        None => progress_indeterminate().fill_width().any(),
    });
    let opens = phase == DownloadPhase::Finished && item.exists;
    let mut body = container(
        row! {
            icon(symbol).extent(Length::px(20.0)).color(palette().text_muted);
            column! {
                text(download_name(item)).label().no_wrap().min_width(0.0);
                progress;
                text(status(item)).caption().muted().no_wrap().min_width(0.0);
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            row_of(actions).gap(Space::HAIR);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
    .fill_width();
    if opens {
        body = body.on_press(move |shell: &mut Shell| shell.open_download(id));
    }
    body
}
