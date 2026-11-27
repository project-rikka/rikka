mod card;
mod lift;
mod wording;

use std::rc::Rc;
use std::sync::Arc;

use animate_library::{Entry, Layout as Placing, Location, Shelf, ThumbnailSize};
use animate_ui::glyph;
use rikka_app::prelude::*;
use rikka_ui::hash_of;
use rikka_widgets::{
    ControlSize, button, lazy_row, lazy_tiles, menu, menu_item, menu_separator, segmented, spinner,
    status_page, text_field,
};

use self::card::{Cell, Look, card};
use crate::browser::Popup;
use crate::shelving::{Desk, Performer};
use crate::thumbnail;

pub const WIDTH: f32 = 284.0;

const MENU: &str = "animate.library.menu";
const MENU_WIDTH: f32 = 260.0;
const CHIP_HEIGHT: f32 = 24.0;
const INSET: f32 = 12.0;
const PENDING_SPINNER: f32 = 14.0;

pub fn library(desk: Desk<'_>) -> impl View {
    let browser = desk.browser();
    let shelf = browser.shelf();
    let shown = browser.shown(desk.catalog());
    let body = match shown.is_empty() {
        true => empty(desk, shelf),
        false => grid(desk, shown),
    };
    container(
        column_of([
            header(desk).any(),
            shelves(desk).any(),
            search(desk).any(),
            folders(desk).any(),
            rule().any(),
            container(body).grow(1.0).fill_width().any(),
            rule().any(),
            loading(desk),
        ])
        .fill(),
    )
    .background(palette().surface)
    .width(WIDTH)
    .fill_height()
}

fn header(desk: Desk<'_>) -> Element {
    let shelf = desk.browser().shelf();
    let mut parts = vec![text(tr!("animate.library.title")).bold().grow(1.0).any()];
    if desk.catalog().pending() > 0 {
        parts.push(
            container(spinner().extent(Length::px(PENDING_SPINNER)))
                .tooltip(tr!("animate.library.reading"))
                .any(),
        );
    }
    if matches!(shelf, Shelf::Motions) {
        parts.push(performing(desk.performer()));
    }
    parts.push(
        glyph(MaterialSymbol::MORE_HORIZ)
            .ghost()
            .on_press(desk.act(|shelving| shelving.browser_mut().toggle_popup(Popup::Menu)))
            .tooltip(tr!("animate.library.options"))
            .build()
            .keyed(MENU),
    );
    if desk.browser().popup() == Some(Popup::Menu) {
        parts.push(options(desk));
    }
    row_of(parts)
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .padding(Inset::edges(INSET, INSET, Space::TIGHT.px(), INSET))
        .fill_width()
        .any()
}

fn performing(performer: Option<Performer>) -> Element {
    let label = match performer {
        Some(performer) => tr!(
            "animate.library.performed_by",
            name = performer.name.as_str()
        ),
        None => tr!("animate.library.performed_by_mannequin"),
    };
    text(label)
        .caption()
        .muted()
        .no_wrap()
        .max_width(WIDTH / 2.0)
        .any()
}

fn options(desk: Desk<'_>) -> Element {
    let shelf = desk.browser().shelf();
    let size = desk.browser().size();
    let mut items = vec![
        menu_item(tr!("animate.library.open_folder"))
            .icon(MaterialSymbol::FOLDER_OPEN)
            .on_press(desk.act(move |shelving| {
                shelving.browser_mut().close_popup();
                let folder = shelving.shelf_folder(shelf).to_path_buf();
                shelving.reveal(&folder);
            })),
        menu_item(tr!(
            "animate.library.add_folder",
            shelf = wording::shelf(shelf).to_string().as_str()
        ))
        .icon(MaterialSymbol::CREATE_NEW_FOLDER)
        .on_press(desk.act(move |shelving| {
            shelving.browser_mut().close_popup();
            shelving.add_location(shelf);
        })),
    ];
    let extra: Vec<Location> = desk
        .locations()
        .extra()
        .iter()
        .filter(|location| match location.layout {
            Placing::Library => true,
            Placing::Shelf(own) => own == shelf,
        })
        .cloned()
        .collect();
    if !extra.is_empty() {
        let missing = desk.catalog().unavailable();
        let removals = extra.into_iter().map(|location| {
            let label = location.root.display().to_string();
            let item = match missing.iter().any(|gone| gone.location == location) {
                true => menu_item(Label::from(label)).icon(MaterialSymbol::WARNING),
                false => menu_item(Label::from(label)),
            };
            item.on_press(desk.act(move |shelving| {
                shelving.browser_mut().close_popup();
                shelving.remove_location(location.clone());
            }))
        });
        items.push(
            menu_item(tr!("animate.library.remove_folder"))
                .submenu(true)
                .items(removals),
        );
    }
    items.push(menu_separator());
    for choice in [
        ThumbnailSize::Small,
        ThumbnailSize::Medium,
        ThumbnailSize::Large,
    ] {
        items.push(
            menu_item(wording::size(choice))
                .selected(choice == size)
                .on_press(desk.act(move |shelving| {
                    shelving.browser_mut().close_popup();
                    shelving.set_thumbnail_size(choice);
                })),
        );
    }
    items.push(menu_separator());
    items.push(
        menu_item(tr!("animate.library.rescan"))
            .icon(MaterialSymbol::REFRESH)
            .on_press(desk.act(|shelving| {
                shelving.browser_mut().close_popup();
                shelving.rescan();
            })),
    );
    menu()
        .items(items)
        .width(MENU_WIDTH)
        .on_dismiss(desk.act(|shelving| shelving.browser_mut().close_popup()))
        .build()
        .keyed((MENU, "menu"))
        .anchored(Anchor::below(MENU))
}

fn shelves(desk: Desk<'_>) -> Element {
    let chosen = Shelf::ALL
        .iter()
        .position(|shelf| *shelf == desk.browser().shelf())
        .unwrap_or(0);
    let tabs = segmented(chosen)
        .glyphs(Shelf::ALL.map(wording::symbol))
        .segments(Shelf::ALL.map(wording::shelf))
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(desk.act_with(|shelving, index: usize| {
            if let Some(shelf) = Shelf::ALL.get(index) {
                shelving.choose_shelf(*shelf);
            }
        }))
        .fill_width();
    container(tabs)
        .padding(Inset::xy(INSET, Space::TIGHT.px()))
        .fill_width()
        .any()
}

fn search(desk: Desk<'_>) -> Element {
    let field = text_field(desk.browser().query())
        .placeholder(tr!(
            "animate.library.search",
            shelf = wording::shelf(desk.browser().shelf()).to_string().as_str()
        ))
        .control_size(ControlSize::Small)
        .on_change(desk.act(|shelving| shelving.browser_mut().typed()))
        .fill_width();
    container(field)
        .padding(Inset::xy(INSET, Space::TIGHT.px()))
        .fill_width()
        .any()
}

fn folders(desk: Desk<'_>) -> Element {
    let browser = desk.browser();
    let folders = desk.catalog().shelf(browser.shelf()).folders();
    if folders.is_empty() {
        return spacer().height(Space::TIGHT.px()).any();
    }
    let chosen = browser.folder().cloned();
    let reach = desk.reach();
    let listed: Rc<[Arc<str>]> = folders.into();
    let count = listed.len() + 1;
    lazy_row(count, move |slot| {
        let Some(index) = slot.index.checked_sub(1) else {
            return chip(
                tr!("animate.library.all_folders"),
                chosen.is_none(),
                reach.act(|shelving| shelving.browser_mut().choose_folder(None)),
            );
        };
        let folder = Arc::clone(&listed[index]);
        let on = chosen.as_ref() == Some(&folder);
        let label = Label::from(folder.to_string());
        chip(
            label,
            on,
            reach.act(move |shelving| {
                shelving
                    .browser_mut()
                    .choose_folder(Some(Arc::clone(&folder)));
            }),
        )
    })
    .gap(Space::TIGHT)
    .padding(Inset::edges(
        Space::TIGHT.px(),
        INSET,
        Space::SNUG.px(),
        INSET,
    ))
    .height(CHIP_HEIGHT + Space::TIGHT.px() + Space::SNUG.px())
    .fill_width()
    .any()
}

fn chip(label: Label, on: bool, press: Handler<(), ()>) -> Element {
    let button = button(text(label).caption().no_wrap())
        .small()
        .focusless()
        .on_press(press);
    match on {
        true => button.primary().height(CHIP_HEIGHT).any(),
        false => button.ghost().height(CHIP_HEIGHT).any(),
    }
}

fn grid(desk: Desk<'_>, shown: Rc<[Arc<Entry>]>) -> Element {
    let browser = desk.browser();
    let tile = tile(browser.size());
    let performer = desk.performer();
    let thumbnailing = desk.thumbnailing();
    let reach = desk.reach();
    let previewed = browser.previewed();
    let feed = previewed.and_then(|id| browser.preview_of(id).cloned());
    let count = shown.len();
    let cells = Rc::clone(&shown);
    let versions = Rc::clone(&shown);
    let names = Rc::clone(&shown);
    let versioned_performer = performer.clone();
    let selected = browser.selected();
    lazy_tiles(Length::px(tile), count, move |slot| {
        let entry = &cells[slot.index];
        let variant = thumbnail::variant(entry, performer.as_ref());
        let feed = feed
            .as_ref()
            .filter(|_| previewed == Some(entry.id()))
            .cloned();
        card(
            Cell {
                entry: Arc::clone(entry),
                look: Look {
                    tile,
                    variant,
                    selected: selected == Some(entry.id()),
                },
                feed,
            },
            thumbnailing.as_ref(),
            reach,
        )
    })
    .memo(move |index| {
        let entry = versions.get(index)?;
        Some(Version {
            key: hash_of(entry.id()),
            revision: hash_of((
                Arc::as_ptr(entry),
                tile.to_bits(),
                previewed == Some(entry.id()),
                selected == Some(entry.id()),
                thumbnail::variant(entry, versioned_performer.as_ref()),
            )),
        })
    })
    .named(move |index| names.get(index).map_or(0, |entry| hash_of(entry.id())))
    .gap(Space::SNUG)
    .padding(Inset::all(INSET))
    .fill()
    .any()
}

fn loading(desk: Desk<'_>) -> Element {
    let chosen = desk
        .browser()
        .selected()
        .and_then(|id| desk.catalog().entry(id))
        .filter(|entry| entry.shelf() == desk.browser().shelf())
        .cloned();
    let name = chosen
        .as_ref()
        .map(|entry| entry.name().to_string())
        .unwrap_or_default();
    let usable = chosen.as_ref().is_some_and(|entry| entry.is_usable());
    let load = button(text(tr!("animate.library.load")))
        .primary()
        .small()
        .focusless()
        .enabled(usable)
        .on_press(desk.act(move |shelving| {
            if let Some(entry) = &chosen {
                shelving.take(Arc::clone(entry));
            }
        }));
    row_of([
        text(name)
            .caption()
            .no_wrap()
            .grow(1.0)
            .min_width(0.0)
            .any(),
        load.any(),
    ])
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(INSET, Space::SNUG.px()))
    .fill_width()
    .any()
}

fn empty(desk: Desk<'_>, shelf: Shelf) -> Element {
    let folder = desk.shelf_folder(shelf).to_path_buf();
    let searching = !desk.browser().query().is_empty() || desk.browser().folder().is_some();
    let message = match searching {
        true => tr!("animate.library.no_matches"),
        false => tr!(
            "animate.library.empty",
            shelf = wording::shelf(shelf).to_string().as_str(),
            folder = folder.display().to_string().as_str()
        ),
    };
    let page = status_page(wording::symbol(shelf), message);
    let page = match searching {
        true => page,
        false => page.action(
            button(text(tr!("animate.library.open_folder")))
                .small()
                .on_press(desk.act(move |shelving| shelving.reveal(&folder))),
        ),
    };
    container(page).padding(Inset::all(INSET)).fill().any()
}

fn tile(size: ThumbnailSize) -> f32 {
    match size {
        ThumbnailSize::Small => 76.0,
        ThumbnailSize::Medium => 120.0,
        ThumbnailSize::Large => WIDTH - INSET * 2.0,
    }
}
