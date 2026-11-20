use std::sync::Arc;

use music_library::{Counts, Folder, FolderKind};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::local::{self, meter, scanned};
use crate::music::Music;
use crate::wording::{albums, songs};

const FOLDER: f32 = 52.0;
const METER: f32 = 220.0;

pub(super) fn view(music: &Music) -> Element {
    let Some(shelf) = local::shelf(music) else {
        return status_page(MaterialSymbol::LIBRARY_MUSIC, tr!("music.local.missing"))
            .fill()
            .any();
    };
    let folders = shelf.read("settings-folders", |library| {
        library.folders().map(Arc::<[Folder]>::from)
    });
    let counts = shelf.read("settings-counts", |library| library.counts());
    let progress = music.scanning();
    column! {
        section(
            tr!("music.library.folders"),
            card(
                column! {
                    async_view(folders, |listed: &Arc<[Folder]>| {
                        column_of(listed.iter().map(folder_row)).gap(Space::HAIR).fill_width()
                    })
                    .loading(|| spinner().extent(Length::px(18.0)));
                    row! {
                        button(
                            row! {
                                icon(MaterialSymbol::CREATE_NEW_FOLDER).extent(Length::px(18.0));
                                text(tr!("music.library.add")).label();
                            }
                            .gap(Space::TIGHT)
                            .align_items(Align::Center),
                        )
                        .secondary()
                        .on_press(|music: &mut Music| music.add_folders());
                    };
                }
                .gap(Space::SNUG)
                .fill_width(),
            ),
        );
        section(
            tr!("music.library.index"),
            card(
                column! {
                    field(
                        tr!("music.library.status"),
                        row! {
                            match progress.scanning {
                                true => column! {
                                    text(scanned(progress)).label().muted().no_wrap();
                                    meter(progress).width(METER);
                                }
                                .gap(Space::TIGHT)
                                .any(),
                                false => text(tr!("music.library.current")).label().muted().any(),
                            };
                            button(tr!("music.library.rescan"))
                                .secondary()
                                .enabled(!progress.scanning)
                                .on_press(|music: &mut Music| music.rescan());
                        }
                        .gap(Space::SNUG)
                        .align_items(Align::Center),
                    );
                    field(
                        tr!("music.library.contents"),
                        async_view(counts, |counts: &Counts| {
                            text(format!(
                                "{} · {} · {}",
                                songs(counts.songs as usize),
                                albums(counts.albums as usize),
                                tr!("music.count.artists", count = &counts.artists.to_string())
                            ))
                            .label()
                            .muted()
                        })
                        .loading(|| spinner().extent(Length::px(16.0))),
                    );
                }
                .gap(Space::SNUG)
                .fill_width(),
            ),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn folder_row(folder: &Folder) -> Element {
    let (symbol, detail) = match (folder.kind, folder.online, folder.remote) {
        (_, false, _) => (MaterialSymbol::FOLDER_OFF, tr!("music.local.offline")),
        (FolderKind::Music, true, _) => {
            (MaterialSymbol::FOLDER_SPECIAL, tr!("music.library.music"))
        }
        (FolderKind::Added, true, true) => {
            (MaterialSymbol::FOLDER_SHARED, tr!("music.library.remote"))
        }
        (FolderKind::Added, true, false) => (MaterialSymbol::FOLDER, tr!("music.library.added")),
    };
    let removed = folder.path.clone();
    row! {
        icon(symbol).extent(Length::px(22.0)).color(match folder.online {
            true => palette().accent,
            false => palette().text_muted,
        });
        column! {
            text(folder.path.display().to_string()).label().medium().no_wrap();
            text(detail).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        if folder.kind == FolderKind::Added {
            button(icon(MaterialSymbol::DELETE).extent(Length::px(18.0)))
                .ghost()
                .icon_only()
                .small()
                .on_press(move |music: &mut Music| music.remove_folder(removed.clone()));
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .height(FOLDER)
    .fill_width()
    .any()
}
