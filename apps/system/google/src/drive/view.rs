use std::collections::HashSet;
use std::ops::Range;

use google_drive::Error;
use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_ui::{DragAction, DragOffer};
use rikka_widgets::{column, row};

use crate::desk::STAGE;
use crate::drive::browser::accepted;
use crate::drive::location::{Location, Origin, Root};
use crate::drive::row::{Column, ViewMode};
use crate::drive::{Asking, DriveState, Listing, Menu, menus};

const SIDEBAR: f32 = 216.0;
const SEARCH: f32 = 220.0;
const CRUMBS_FLOOR: f32 = 96.0;
const CRUMBS_SHOWN: usize = 4;
const MENU_WIDTH: f32 = 264.0;
const MARK: &str = "google-drive-mark";
const MENU_KEY: &str = "google-drive-menu";
const ASKING: &str = "google-drive-asking";
const NEW_BUTTON: &str = "google-drive-new";
const SORT_BUTTON: &str = "google-drive-sort";

pub(crate) fn view(drive: &DriveState) -> Element {
    let mut layers = vec![
        column! {
            bar(drive);
            rule();
            row! {
                sidebar(drive);
                rule_upright();
                listing(drive);
            }
            .grow(1.0)
            .fill_width();
            status(drive);
        }
        .fill()
        .any(),
    ];
    if let Some((menu, key)) = drive.menu() {
        layers.push(overlay_menu(drive, menu, key));
    }
    if let Some(asking) = drive.asking() {
        let (title, message, accept) = match asking {
            Asking::DeleteForever(ids) => (
                tr!("google.drive.delete.title"),
                tr!("google.drive.delete.message", count = ids.len()),
                tr!("google.drive.menu.delete-forever"),
            ),
            Asking::EmptyTrash => (
                tr!("google.drive.empty-trash.title"),
                tr!("google.drive.empty-trash.message"),
                tr!("google.drive.menu.empty-trash"),
            ),
        };
        layers.extend(
            confirm_prompt(
                title,
                message,
                accept,
                |drive: &mut DriveState, confirmed: bool| drive.answer(confirmed),
            )
            .keyed(ASKING)
            .centered_on(STAGE)
            .with_scrim(),
        );
    }
    column_of(layers).fill().any()
}

pub(crate) fn failure(error: &Error) -> String {
    match error {
        Error::Missing => tr!("google.drive.failure.missing").to_string(),
        Error::StorageFull => tr!("google.drive.failure.full").to_string(),
        Error::RateLimited => tr!("google.drive.failure.busy").to_string(),
        Error::NotDownloadable => tr!("google.drive.failure.no-content").to_string(),
        other => other.to_string(),
    }
}

fn bar(drive: &DriveState) -> impl View {
    let step = |symbol, command: Command, enabled: bool| {
        tool_button(symbol, command_label(command))
            .shortcut(command.chord())
            .control_size(ControlSize::Medium)
            .enabled(enabled)
    };
    let navigation = tool_group([
        step(
            MaterialSymbol::ARROW_BACK,
            Command::Back,
            drive.can_go_back(),
        )
        .on_press(DriveState::back),
        step(
            MaterialSymbol::ARROW_FORWARD,
            Command::Forward,
            drive.can_go_forward(),
        )
        .on_press(DriveState::forward),
        step(
            MaterialSymbol::ARROW_UPWARD,
            Command::Up,
            drive.location().parent().is_some(),
        )
        .on_press(DriveState::up),
        step(MaterialSymbol::REFRESH, Command::Refresh, true).on_press(DriveState::refresh),
    ])
    .gap(Space::HAIR)
    .priority(ToolPriority::Pinned);
    let search = text_field(drive.search())
        .placeholder(tr!("google.drive.search.field"))
        .claims_focus(drive.is_seeking())
        .on_submit(DriveState::sought)
        .on_blur(DriveState::seek_ended)
        .width(SEARCH);
    let (toggle, other, hint) = match drive.mode() {
        ViewMode::Icons => (
            MaterialSymbol::VIEW_LIST,
            ViewMode::Details,
            tr!("google.drive.view.details"),
        ),
        ViewMode::Details => (
            MaterialSymbol::GRID_VIEW,
            ViewMode::Icons,
            tr!("google.drive.view.icons"),
        ),
    };
    let opener = |menu: Menu, symbol, label: Label, anchor: &'static str| {
        tool_item(
            button(icon(symbol))
                .ghost()
                .icon_only()
                .focusless()
                .on_press(move |drive: &mut DriveState| drive.show_menu(menu))
                .tooltip(label)
                .keyed(anchor),
        )
    };
    toolbar()
        .gap(Space::SNUG)
        .padding(Inset::xy(10.0, 8.0))
        .control_size(ControlSize::Medium)
        .group(navigation)
        .flexible(crumbs(drive.location()), CRUMBS_FLOOR)
        .group(tool_group([tool_item(search)]).priority(ToolPriority::Pinned))
        .group(tool_group([
            tool_item(
                tool_button(toggle, hint)
                    .control_size(ControlSize::Medium)
                    .on_press(move |drive: &mut DriveState| drive.view_as(other)),
            ),
            opener(
                Menu::Sort,
                MaterialSymbol::SORT,
                tr!("google.drive.sort.title"),
                SORT_BUTTON,
            ),
            opener(
                Menu::New,
                MaterialSymbol::ADD,
                tr!("google.drive.menu.new"),
                NEW_BUTTON,
            ),
        ]))
}

fn crumbs(location: &Location) -> Element {
    let names = location.names();
    let last = names.len() - 1;
    let first = names.len().saturating_sub(CRUMBS_SHOWN);
    let mut chips = Vec::new();
    if first > 0 {
        chips.push(text("…").muted().any());
    }
    for (depth, name) in names.into_iter().enumerate().skip(first) {
        if depth > first || first > 0 {
            chips.push(
                icon(MaterialSymbol::CHEVRON_RIGHT)
                    .extent(Length::px(16.0))
                    .color(palette().text_muted)
                    .any(),
            );
        }
        chips.push(match depth == last {
            true => container(text(name).label().semibold().no_wrap())
                .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                .min_width(0.0)
                .any(),
            false => button(text(name).label().no_wrap())
                .ghost()
                .small()
                .focusless()
                .on_press(move |drive: &mut DriveState| drive.go_crumb(depth))
                .any(),
        });
    }
    row_of(chips)
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .grow(1.0)
        .min_width(CRUMBS_FLOOR)
        .build()
        .clipped()
}

fn sidebar(drive: &DriveState) -> impl View {
    let current = match &drive.location().origin {
        Origin::Root(root) => Root::ALL.iter().position(|each| each == root),
        Origin::Search(_) => None,
    };
    column! {
        nav_list(0)
            .items(Root::ALL.map(|root| nav_item(root.title()).icon(root.symbol())))
            .selected(current)
            .on_select(|drive: &mut DriveState, index: usize| drive.go_root(index))
            .grow(1.0)
            .fill_width();
        if let Some(storage) = drive.storage() {
            column! {
                if let Some(limit) = storage.limit.filter(|limit| *limit > 0) {
                    progress(storage.usage as f32 / limit as f32).fill_width();
                    text(tr!(
                        "google.drive.storage.of",
                        used = format_bytes(storage.usage).as_str(),
                        limit = format_bytes(limit).as_str()
                    ))
                    .caption()
                    .muted();
                } else {
                    text(tr!(
                        "google.drive.storage.used",
                        used = format_bytes(storage.usage).as_str()
                    ))
                    .caption()
                    .muted();
                }
            }
            .gap(Space::TIGHT)
            .padding(Inset::all(Space::NORMAL))
            .fill_width();
        }
    }
    .padding(Inset::xy(Space::SNUG, Space::SNUG))
    .width(SIDEBAR)
    .fill_height()
}

fn listing(drive: &DriveState) -> Element {
    let content = match (drive.listing(), drive.count()) {
        (Listing::Loading, 0) => container(spinner().extent(Length::px(24.0)))
            .center()
            .fill()
            .any(),
        (Listing::Failed(reason), _) => status_page(MaterialSymbol::CLOUD_OFF, reason.clone())
            .action(button(text(tr!("google.drive.retry"))).on_press(DriveState::refresh))
            .any(),
        (Listing::Ready, 0) => empty(drive.location()),
        _ => browser(drive),
    };
    let takes = drive.takes_files();
    drop_zone(
        row! {
            spacer().width(0.0).fill_height().build().keyed(MARK);
            page_stack(drive.depth(), content.keyed(drive.page()))
                .named(drive.page())
                .grow(1.0)
                .fill_height();
        }
        .grow(1.0)
        .fill_height(),
    )
    .content_first(true)
    .accepts(move |offer: &DragOffer| takes.then(|| accepted(offer)).flatten())
    .on_drop(
        |drive: &mut DriveState, offer: DragOffer, action: DragAction| {
            drive.dropped(None, offer, action);
        },
    )
    .grow(1.0)
    .fill_height()
    .any()
}

fn browser(drive: &DriveState) -> Element {
    let browser = drive
        .browser()
        .on_open(|drive: &mut DriveState, index: usize| drive.open(index))
        .on_select(
            |drive: &mut DriveState, chosen: HashSet<usize>, focus: Option<usize>| {
                drive.select(chosen, focus);
            },
        )
        .on_context(|drive: &mut DriveState, at: Point| drive.show_menu(Menu::Context(at)))
        .on_sort(|drive: &mut DriveState, column: Column| drive.sort_by(column))
        .on_visible(|drive: &mut DriveState, range: Range<usize>| drive.visible(range))
        .on_rename(|drive: &mut DriveState, name: String| drive.rename_to(&name))
        .on_rename_cancel(DriveState::end_rename);
    match drive.takes_files() {
        true => browser.on_drop_into(
            |drive: &mut DriveState, folder: usize, offer: DragOffer, action: DragAction| {
                drive.dropped(Some(folder), offer, action);
            },
        ),
        false => browser,
    }
    .build()
}

fn empty(location: &Location) -> Element {
    let (symbol, message) = match (&location.origin, location.trail.is_empty()) {
        (Origin::Search(_), true) => (MaterialSymbol::SEARCH_OFF, tr!("google.drive.empty.search")),
        (Origin::Root(Root::Trash), true) => {
            (MaterialSymbol::DELETE, tr!("google.drive.empty.trash"))
        }
        (Origin::Root(Root::Starred), true) => {
            (MaterialSymbol::STAR, tr!("google.drive.empty.starred"))
        }
        (Origin::Root(Root::SharedWithMe | Root::SharedDrives), true) => {
            (MaterialSymbol::GROUP, tr!("google.drive.empty.shared"))
        }
        _ => (
            MaterialSymbol::FOLDER_OPEN,
            tr!("google.drive.empty.folder"),
        ),
    };
    let page = status_page(symbol, message);
    match location.folder() {
        Some(_) => page.action(
            row! {
                button(text(tr!("google.drive.menu.new-folder")))
                    .on_press(DriveState::new_folder);
                button(text(tr!("google.drive.menu.upload")))
                    .primary()
                    .on_press(DriveState::upload);
            }
            .gap(Space::SNUG),
        ),
        None => page,
    }
    .any()
}

fn status(drive: &DriveState) -> impl View {
    let selected = drive.selected().len();
    let summary = match selected {
        0 => tr!("google.drive.items", count = drive.count()),
        selected => tr!(
            "google.drive.selected",
            selected = selected,
            count = drive.count()
        ),
    };
    column! {
        if let Some(notice) = drive.notice() {
            let line = banner(notice.text.clone());
            let line = match notice.danger {
                true => line,
                false => line.symbol(MaterialSymbol::INFO).tint(palette().accent),
            };
            let line = match (&notice.folder, notice.restores.is_empty()) {
                (Some(folder), _) => {
                    let folder = folder.clone();
                    line.on_action(
                        tr!("google.drive.show-folder"),
                        move |drive: &mut DriveState| drive.reveal_download(&folder),
                    )
                }
                (None, false) => line.on_action(tr!("google.drive.undo"), DriveState::undo_notice),
                (None, true) => line,
            };
            container(line.on_dismiss(tr!("google.dismiss"), DriveState::dismiss_notice))
                .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
                .fill_width();
        }
        rule();
        row! {
            text(summary).caption().muted().no_wrap();
            spacer().grow(1.0);
            if let Some(transfer) = drive.transfer() {
                row! {
                    match transfer.fraction {
                        Some(fraction) => progress(fraction).width(140.0).any(),
                        None => spinner().extent(Length::px(14.0)).any(),
                    };
                    text(transfer.label.clone()).caption().no_wrap();
                    button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                        .ghost()
                        .small()
                        .icon_only()
                        .on_press(DriveState::cancel_transfer)
                        .tooltip(tr!("google.cancel"));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .transition(fade());
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .padding(Inset::xy(12.0, 6.0))
        .fill_width();
    }
    .fill_width()
}

fn overlay_menu(drive: &DriveState, shown: Menu, key: u64) -> Element {
    let anchor = match shown {
        Menu::Context(at) => Anchor::new(MARK, AnchorEdge::BesideTop).offset(at.x, at.y),
        Menu::New => Anchor::below(NEW_BUTTON).offset(0.0, 4.0),
        Menu::Sort => Anchor::below(SORT_BUTTON).offset(0.0, 4.0),
    };
    menu()
        .items(menus::items(drive, shown))
        .width(MENU_WIDTH)
        .on_dismiss(DriveState::close_menu)
        .build()
        .keyed((MENU_KEY, key))
        .anchored(anchor)
}
