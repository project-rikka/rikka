use music_library::Progress;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::music::Music;

const PROGRESS: f32 = 280.0;

pub(crate) fn scanned(progress: Progress) -> Label {
    match progress.to_read {
        0 => tr!("music.local.finding", count = &progress.found.to_string()),
        total => tr!(
            "music.local.scanning",
            read = &progress.read.to_string(),
            total = &total.to_string()
        ),
    }
}

pub(crate) fn meter(scan: Progress) -> rikka_widgets::Progress {
    match scan.to_read {
        0 => progress_indeterminate(),
        total => progress(scan.read as f32 / total as f32),
    }
}

pub(crate) fn scan_line(progress: Progress) -> Option<Element> {
    progress.scanning.then(|| {
        column! {
            row! {
                spinner().extent(Length::px(14.0));
                text(scanned(progress)).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            meter(progress).control_size(ControlSize::Small).fill_width();
        }
        .gap(Space::TIGHT)
        .padding(Inset::xy(Space::NORMAL, Space::SNUG))
        .fill_width()
        .any()
    })
}

pub(super) fn empty(progress: Progress) -> Element {
    if progress.scanning {
        return column! {
            spinner();
            text(tr!("music.local.filling")).heading().bold();
            text(scanned(progress)).label().muted();
            meter(progress).width(PROGRESS);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .justify(Justify::Center)
        .fill()
        .any();
    }
    status_page(MaterialSymbol::LIBRARY_MUSIC, tr!("music.local.empty"))
        .action(
            row! {
                button(
                    row! {
                        icon(MaterialSymbol::CREATE_NEW_FOLDER).extent(Length::px(18.0));
                        text(tr!("music.library.add")).label().semibold();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .primary()
                .on_press(|music: &mut Music| music.add_folders());
                button(text(tr!("music.library.open")))
                    .secondary()
                    .on_press(|music: &mut Music| music.open_library_settings());
            }
            .gap(Space::SNUG),
        )
        .fill()
        .any()
}
