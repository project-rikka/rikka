use std::path::{Path, PathBuf};
use std::sync::Arc;

use music_core::Track;
use music_library::{Folder, FolderKind, Library, Listing, Subfolder};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::blocks::{Leading, Listening, UNAVAILABLE};
use crate::local::empty::empty;
use crate::local::fresh::Shelf;
use crate::local::title_bar;
use crate::music::{Music, Start};
use crate::nav::Place;
use crate::pages::{Block, fetched, page_of};
use crate::wording::songs;

const ROW: f32 = 60.0;
const BADGE: f32 = 40.0;

struct Root {
    folder: Folder,
    tracks: u32,
}

struct FolderContents {
    listing: Listing,
    tracks: Arc<[Track]>,
    total: u32,
}

pub(super) fn roots(music: &Music, shelf: &Shelf) -> Element {
    let scanning = music.scanning();
    let loading = shelf.read("folders", |library| {
        library
            .folders()?
            .into_iter()
            .map(|folder| {
                let tracks = match folder.online {
                    true => library.count_under(&folder.path)?,
                    false => 0,
                };
                Ok(Root { folder, tracks })
            })
            .collect::<music_library::Result<Vec<Root>>>()
            .map(Arc::<[Root]>::from)
    });
    fetched(loading, move |listed: &Arc<[Root]>| {
        if listed.iter().all(|root| root.tracks == 0) && scanning.scanning {
            return empty(scanning);
        }
        let shown = Arc::clone(listed);
        column! {
            title_bar(tr!("music.nav.folders"), songs(shown.iter().map(|root| root.tracks as usize).sum()), add_button());
            lazy_column(shown.len(), move |slot| root_row(&shown[slot.index]))
                .estimate(ROW)
                .padding(Inset::edges(Space::NONE, Space::LOOSE, Space::WIDE, Space::LOOSE))
                .grow(1.0)
                .fill_width();
        }
        .fill()
        .any()
    })
}

pub(super) fn folder(shelf: &Shelf, root: &Path, path: &Path, listening: &Listening) -> Element {
    let folder_path = path.to_path_buf();
    let loading = shelf.read(("folder", path), move |library| {
        folder_contents(library, &folder_path)
    });
    let (root, path) = (root.to_path_buf(), path.to_path_buf());
    let listening = listening.clone();
    fetched(loading, move |contents: &FolderContents| {
        page_of(blocks(&root, &path, contents), listening.clone())
    })
}

fn folder_contents(library: &Library, path: &Path) -> music_library::Result<FolderContents> {
    let listing = library.folder(path)?;
    Ok(FolderContents {
        tracks: music_local::tracks(&listing.tracks).into(),
        total: counted(&listing),
        listing,
    })
}

fn counted(listing: &Listing) -> u32 {
    listing
        .folders
        .iter()
        .map(|inner| inner.tracks)
        .sum::<u32>()
        + listing.tracks.len() as u32
}

fn add_button() -> impl View {
    button(
        row! {
            icon(MaterialSymbol::CREATE_NEW_FOLDER).extent(Length::px(18.0));
            text(tr!("music.library.add")).label();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .secondary()
    .on_press(|music: &mut Music| music.add_folders())
}

fn root_row(root: &Root) -> Element {
    let folder = &root.folder;
    let symbol = match (folder.kind, folder.online, folder.remote) {
        (_, false, _) => MaterialSymbol::FOLDER_OFF,
        (_, true, true) => MaterialSymbol::FOLDER_SHARED,
        (FolderKind::Music, true, false) => MaterialSymbol::FOLDER_SPECIAL,
        (FolderKind::Added, true, false) => MaterialSymbol::FOLDER,
    };
    let detail = match folder.online {
        true => songs(root.tracks as usize).to_string(),
        false => tr!("music.local.offline").to_string(),
    };
    let place = Place::Folder {
        root: folder.path.clone(),
        path: folder.path.clone(),
    };
    let body = entry(
        symbol,
        named(&folder.path),
        folder.path.display().to_string(),
        detail,
    )
    .on_press(move |music: &mut Music| music.go(place.clone()))
    .height(ROW)
    .fill_width();
    match folder.online {
        true => body.any(),
        false => body.opacity(UNAVAILABLE).any(),
    }
}

fn entry(symbol: MaterialSymbol, title: String, under: String, trailing: String) -> Container {
    container(
        row! {
            container(icon(symbol).extent(Length::px(22.0)).color(palette().accent))
                .center()
                .background(palette().accent.with_alpha(0.12))
                .radius(Corner::Item)
                .size(Length::px(BADGE), Length::px(BADGE));
            column! {
                text(title).label().medium().no_wrap();
                text(under).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            text(trailing).caption().muted().no_wrap();
            icon(MaterialSymbol::CHEVRON_RIGHT).extent(Length::px(20.0)).color(palette().text_muted);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item)
}

fn blocks(root: &Path, path: &Path, contents: &FolderContents) -> Vec<Block> {
    let mut blocks = Vec::new();
    let (crumbs_root, crumbs_path) = (root.to_path_buf(), path.to_path_buf());
    blocks.push(Block::line(move || crumbs(&crumbs_root, &crumbs_path)));
    let (title, folder, total) = (named(path), path.to_path_buf(), contents.total);
    blocks.push(Block::line(move || {
        title_bar(
            title.clone(),
            songs(total as usize),
            starts(&folder, total > 0),
        )
        .any()
    }));
    if contents.listing.tracks.is_empty() && contents.listing.folders.is_empty() {
        blocks.push(Block::line(|| {
            status_page(MaterialSymbol::FOLDER_OFF, tr!("music.local.folder-empty"))
                .height(240.0)
                .fill_width()
                .any()
        }));
        return blocks;
    }
    for inner in &contents.listing.folders {
        let (root, inner) = (root.to_path_buf(), inner.clone());
        blocks.push(Block::line(move || subfolder_row(&root, &inner)));
    }
    if !contents.tracks.is_empty() {
        if !contents.listing.folders.is_empty() {
            blocks.push(Block::Heading(tr!("music.search.songs")));
        }
        blocks.push(Block::tracks(&contents.tracks, Leading::Cover));
    }
    blocks
}

fn starts(folder: &Path, playable: bool) -> impl View {
    let (played, shuffled, queued) = (
        folder.to_path_buf(),
        folder.to_path_buf(),
        folder.to_path_buf(),
    );
    row! {
        button(
            row! {
                icon(MaterialSymbol::PLAY_ARROW).filled().extent(Length::px(18.0));
                text(tr!("music.play")).label().semibold();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .enabled(playable)
        .on_press(move |music: &mut Music| music.play_folder(played.clone(), Start::Play));
        button(
            row! {
                icon(MaterialSymbol::SHUFFLE).extent(Length::px(18.0));
                text(tr!("music.shuffle")).label();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .secondary()
        .enabled(playable)
        .on_press(move |music: &mut Music| music.play_folder(shuffled.clone(), Start::Shuffle));
        button(icon(MaterialSymbol::ADD_TO_QUEUE).extent(Length::px(18.0)))
            .secondary()
            .icon_only()
            .enabled(playable)
            .on_press(move |music: &mut Music| music.play_folder(queued.clone(), Start::Queue));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn subfolder_row(root: &Path, inner: &Subfolder) -> Element {
    let place = Place::Folder {
        root: root.to_path_buf(),
        path: inner.path.clone(),
    };
    let played = inner.path.clone();
    row! {
        entry(
            MaterialSymbol::FOLDER,
            inner.name.clone(),
            songs(inner.tracks as usize).to_string(),
            String::new(),
        )
        .on_press(move |music: &mut Music| music.go(place.clone()))
        .grow(1.0)
        .min_width(0.0);
        button(icon(MaterialSymbol::PLAY_ARROW).filled().extent(Length::px(18.0)))
            .ghost()
            .icon_only()
            .small()
            .on_press(move |music: &mut Music| music.play_folder(played.clone(), Start::Play));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .height(ROW)
    .fill_width()
    .any()
}

fn crumbs(root: &Path, path: &Path) -> Element {
    let mut trail: Vec<(String, PathBuf)> = vec![(named(root), root.to_path_buf())];
    if let Ok(inner) = path.strip_prefix(root) {
        let mut at = root.to_path_buf();
        for part in inner.components() {
            at.push(part);
            trail.push((part.as_os_str().to_string_lossy().into_owned(), at.clone()));
        }
    }
    let last = trail.len() - 1;
    let mut parts: Vec<Element> = vec![
        button(icon(MaterialSymbol::FOLDER_OPEN).extent(Length::px(16.0)))
            .ghost()
            .icon_only()
            .small()
            .on_press(|music: &mut Music| music.root(Place::Folders))
            .build(),
    ];
    for (at, (name, place)) in trail.into_iter().enumerate() {
        parts.push(
            icon(MaterialSymbol::CHEVRON_RIGHT)
                .extent(Length::px(16.0))
                .color(palette().text_muted)
                .build(),
        );
        let crumb = match at == last {
            true => text(name).label().semibold().no_wrap().any(),
            false => {
                let going = Place::Folder {
                    root: root.to_path_buf(),
                    path: place,
                };
                button(text(name).label().no_wrap())
                    .ghost()
                    .small()
                    .on_press(move |music: &mut Music| music.go(going.clone()))
                    .any()
            }
        };
        parts.push(crumb);
    }
    row_of(parts)
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .wrap_items()
        .padding(Inset::edges(
            Space::ROOMY,
            Space::NONE,
            Space::NONE,
            Space::NONE,
        ))
        .fill_width()
        .any()
}

fn named(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
