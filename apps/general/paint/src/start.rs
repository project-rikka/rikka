use std::path::{Path, PathBuf};

use rikka_app::prelude::*;
use rikka_ui::ImageSource;
use rikka_widgets::{column, row};
use rikka_xdg::RecentFile;

use crate::paint::Paint;
use crate::thumbnail::{source_of, thumbnail};

const EMBLEM: f32 = 64.0;
const PREVIEW: f32 = 136.0;
const PREVIEW_PIXELS: u32 = 272;
const TILE: f32 = 168.0;
const WIDEST: f32 = 920.0;
const GLYPH: f32 = 32.0;

pub(crate) fn page(paint: &Paint) -> impl View {
    let recent = paint.recent();
    let tiles: Vec<Element> = recent.iter().map(|file| tile(file).build()).collect();
    let content = column! {
        icon(MaterialSymbol::PALETTE)
            .extent(Length::px(EMBLEM))
            .color(palette().accent);
        text(tr!("paint.title")).title();
        button(
            row! {
                icon(MaterialSymbol::FOLDER_OPEN);
                text(tr!("paint.start.open"));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .primary()
        .large()
        .on_press(Paint::show_open_dialog);
        if !tiles.is_empty() {
            column! {
                text(tr!("paint.start.recent")).bold();
                row_of(tiles).wrap_items().gap(Space::SNUG);
            }
            .gap(Space::NORMAL)
            .margin(Inset::edges(Space::LOOSE, Space::NONE, Space::NONE, Space::NONE))
            .fill_width();
        }
    }
    .gap(Space::ROOMY)
    .align_items(Align::Center)
    .padding(Space::WIDE)
    .max_width(WIDEST)
    .fill_width();
    scrollable(container(content).align_x(Align::Center).fill_width()).fill()
}

fn tile(file: &RecentFile) -> impl View {
    let path = file.path.clone();
    let name = file
        .path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let folder = file
        .path
        .parent()
        .and_then(Path::file_name)
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let face = column! {
        container(preview(&file.path)).center().size(PREVIEW, PREVIEW);
        text(name).caption().no_wrap().fill_width();
        text(folder).caption().muted().no_wrap().fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width();
    container(face)
        .padding(Space::SNUG)
        .radius(Corner::Item)
        .on_press(move |paint: &mut Paint| paint.open_paths(vec![path.clone()]))
        .focusless()
        .width(TILE)
}

fn preview(path: &Path) -> impl View {
    let owned: PathBuf = path.to_path_buf();
    let fetched = fetch_blocking(("paint.recent", owned.clone()), move || {
        paint_document::preview(&owned, PREVIEW_PIXELS)
            .map(source_of)
            .ok_or("no preview")
    });
    async_view(fetched, |source: &ImageSource| {
        thumbnail(source.clone(), PREVIEW)
    })
    .failed(|_| {
        icon(MaterialSymbol::IMAGE)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted)
    })
}
