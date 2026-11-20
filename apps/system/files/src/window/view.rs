use std::collections::HashSet;
use std::ops::Range;

use files_core::{Location, SortKey, ViewMode};
use rikka_file_operations::Resolution;

use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_ui::{DragAction, DragOffer, Element, View};
use rikka_widgets::column;

use crate::browser::accept_files;
use crate::frame;
use crate::places::Places;
use crate::tab::Tab;
use crate::tabs::Tabs;
use crate::window::jobs::{self, Jobs};
use crate::window::menus::{Menu, Menus, Shown};
use crate::window::{Asking, Compressing, Converting, Dialog, TABS, WindowState, clipboard};
use crate::wording;

const MENU_KEY: &str = "files-menu";
const DIALOG_KEY: &str = "files-dialog";
const CONFLICT_KEY: &str = "files-conflict";
const TOOLS_BUTTON: &str = "files-tools";
const SORT_BUTTON: &str = "files-sort";

pub(crate) fn view(window: &WindowState) -> Element {
    if window.tabs.is_empty() {
        return Element::empty();
    }
    let question = window
        .jobs
        .showing()
        .and_then(|showing| showing.question.as_ref());
    frame::stage(
        column! {
            bar(window);
            rule();
            row! {
                sidebar(window);
                listing(window);
            }
            .grow(1.0);
            status(window);
            if let Some(shown) = window.menus.open() {
                overlay_menu(window, shown);
            }
            if let Some(shown) = window.dialog.showing() {
                for layer in dialog(shown, window.dialog.key()) {
                    layer;
                }
            }
            if let Some(question) = question {
                for layer in collision(question) {
                    layer;
                }
            }
        }
        .fill(),
    )
}

pub(crate) fn strip(window: &WindowState) -> Option<Element> {
    if window.tabs.is_empty() {
        return None;
    }
    Some(
        tabs(window.tabs.active_index())
            .items(window.tabs.iter().map(Tab::strip))
            .in_caption()
            .on_select(|window: &mut WindowState, index| window.select_tab(index))
            .on_close(|window: &mut WindowState, index| window.close_tab(index))
            .on_new(WindowState::new_tab)
            .transfers(
                Transfer::group(TABS)
                    .take(|window: &mut WindowState, index| window.lift_tab(index))
                    .insert(|window: &mut WindowState, index, tab| window.land_tab(index, tab))
                    .tear_off(WindowState::tear_off_tab, false),
            )
            .on_finished(WindowState::close_if_no_tabs)
            .build(),
    )
}

fn bar(window: &WindowState) -> impl View {
    let tab = window.tabs.active();
    let can_go_up = window.services.vfs.parent_of(tab.at()).is_some();
    let navigation = tool_group([
        frame::step(
            MaterialSymbol::ARROW_BACK,
            Command::Back,
            WindowState::back,
            tab.history().can_go_back(),
        ),
        frame::step(
            MaterialSymbol::ARROW_FORWARD,
            Command::Forward,
            WindowState::forward,
            tab.history().can_go_forward(),
        ),
        frame::step(
            MaterialSymbol::ARROW_UPWARD,
            Command::Up,
            WindowState::up,
            can_go_up,
        ),
        frame::step(
            MaterialSymbol::REFRESH,
            Command::Refresh,
            WindowState::refresh_active,
            true,
        ),
    ])
    .gap(Space::HAIR)
    .priority(ToolPriority::Pinned);
    let search = text_field(tab.finding().text())
        .placeholder(tr!("files.toolbar.search"))
        .claims_focus(window.seeking())
        .on_change(WindowState::search)
        .on_blur(WindowState::sought)
        .width(200.0);
    toolbar()
        .gap(Space::SNUG)
        .padding(Inset::xy(10.0, 8.0))
        .control_size(ControlSize::Medium)
        .group(navigation)
        .flexible(
            frame::path_bar(
                tab.at(),
                tab.editing_path(),
                |window: &mut WindowState, at: Location| window.navigate_active(at),
                |tabs: &mut Tabs| tabs.active_mut().edit_path(true),
                |tabs: &mut Tabs| tabs.active_mut().edit_path(false),
                WindowState::path,
            ),
            frame::PATH_MIN,
        )
        .group(tool_group([tool_item(search)]).priority(ToolPriority::Pinned))
        .group(tool_group([
            view_tool(tab.mode()),
            menu_tool(
                window,
                (Menu::Sort, SORT_BUTTON),
                MaterialSymbol::SORT,
                tr!("files.toolbar.sort"),
            ),
            menu_tool(
                window,
                (Menu::Tools, TOOLS_BUTTON),
                MaterialSymbol::MORE_VERT,
                tr!("files.toolbar.tools"),
            ),
        ]))
}

fn view_tool(mode: ViewMode) -> ToolItem {
    const MODES: [ViewMode; 2] = [ViewMode::Icons, ViewMode::Details];
    let (symbol, other) = match mode {
        ViewMode::Icons => (MaterialSymbol::VIEW_LIST, ViewMode::Details),
        ViewMode::Details => (MaterialSymbol::GRID_VIEW, ViewMode::Icons),
    };
    let toggle = tool_button(symbol, wording::view_mode(other))
        .control_size(ControlSize::Medium)
        .on_press(move |window: &mut WindowState| window.view_as(other));
    tool_item(toggle).overflow([menu_item(tr!("files.toolbar.view"))
        .icon(MaterialSymbol::GRID_VIEW)
        .choices(
            MODES.map(wording::view_mode),
            MODES.iter().position(|each| *each == mode),
            |window: &mut WindowState, index: usize| {
                if let Some(mode) = MODES.get(index) {
                    window.view_as(*mode);
                }
            },
        )])
}

fn menu_tool(
    window: &WindowState,
    (menu, anchor): (Menu, &'static str),
    symbol: MaterialSymbol,
    label: Label,
) -> ToolItem {
    let opener = button(icon(symbol))
        .ghost()
        .icon_only()
        .focusless()
        .on_press(move |menus: &mut Menus| menus.show(menu))
        .tooltip(label.clone())
        .keyed(anchor);
    tool_item(opener).overflow([menu_item(label).icon(symbol).items(window.menu_items(menu))])
}

fn sidebar(window: &WindowState) -> impl View {
    frame::places(
        Places::new(window.services.rows(), Some(window.tabs.active().at()))
            .on_open(|window: &mut WindowState, index| window.open_place(index, false))
            .on_open_tab(|window: &mut WindowState, index| window.open_place(index, true))
            .on_trailing(|window: &mut WindowState, index| window.detach(index))
            .on_context(|menus: &mut Menus, index, at| menus.show(Menu::Places { index, at }))
            .on_reorder(
                |window: &mut WindowState, moved: Location, before: Location| {
                    window.reorder(&moved, &before);
                },
            ),
    )
}

fn listing(window: &WindowState) -> impl View {
    let tab = window.tabs.active();
    let takes_files = tab.folder_allows().create;
    let browser = tab
        .browser()
        .on_open(|window: &mut WindowState, index| window.open(index))
        .on_open_tab(|window: &mut WindowState, index| window.open_beside(index))
        .on_select(
            |window: &mut WindowState, chosen: HashSet<usize>, focus: Option<usize>| {
                window.select(chosen, focus);
            },
        )
        .on_context(|menus: &mut Menus, at| {
            let pastable = clipboard::holds_files();
            menus.show(Menu::Context { at, pastable });
        })
        .on_sort(|window: &mut WindowState, key: SortKey| window.sort_listing(key))
        .on_visible(|window: &mut WindowState, range: Range<usize>| window.visible(range))
        .on_rename(|window: &mut WindowState, name: String| window.rename_to(&name))
        .on_rename_cancel(|tabs: &mut Tabs| tabs.active_mut().end_rename());
    let browser = match takes_files {
        true => browser.on_drop_into(
            |window: &mut WindowState, folder: usize, offer: DragOffer, action: DragAction| {
                window.drop_into(Some(folder), offer, action);
            },
        ),
        false => browser,
    };
    drop_zone(frame::listing(tab, window.tabs.active_index(), browser))
        .content_first(true)
        .accepts(move |offer: &DragOffer| takes_files.then(|| accept_files(offer)).flatten())
        .on_drop(
            |window: &mut WindowState, offer: DragOffer, action: DragAction| {
                window.drop_into(None, offer, action);
            },
        )
        .grow(1.0)
        .fill_height()
}

fn status(window: &WindowState) -> impl View {
    let line = match window.jobs.showing() {
        Some(_) => jobs::line(&window.jobs),
        None => match &window.tabs.active().failure {
            Some(failure) => row! {
                banner(failure.as_str())
                    .on_dismiss(tr!("files.job.dismiss"), WindowState::dismiss_failure);
            },
            None => row! {
                text(window.summary()).caption();
            },
        },
    };

    column! {
        rule();
        container(
            row! {
                line.gap(Space::NORMAL).align_items(Align::Center);
                spacer().grow(1.0);
            }
            .align_items(Align::Center),
        )
        .padding(Inset::xy(12.0, 6.0))
        .fill_width();
    }
}

fn overlay_menu(window: &WindowState, shown: Shown) -> Element {
    let anchor = match shown.menu {
        Menu::Context { at, .. } => {
            Anchor::new(frame::MARK, AnchorEdge::BesideTop).offset(at.x, at.y)
        }
        Menu::Places { at, .. } => {
            Anchor::new(frame::PLACES_MARK, AnchorEdge::BesideTop).offset(at.x, at.y)
        }
        Menu::Tools => Anchor::below(TOOLS_BUTTON).offset(0.0, 4.0),
        Menu::Sort => Anchor::below(SORT_BUTTON).offset(0.0, 4.0),
        Menu::OpenWith => Anchor::new(frame::MARK, AnchorEdge::BesideTop).offset(24.0, 24.0),
    };
    menu()
        .items(window.menu_items(shown.menu))
        .width(272.0)
        .on_dismiss(Menus::close)
        .build()
        .keyed((MENU_KEY, shown.key))
        .anchored(anchor)
}

fn dialog(dialog: &Dialog, key: u64) -> Vec<Element> {
    let body = match dialog {
        Dialog::Properties { heading, lines } => column! {
            text(heading.as_str()).semibold();
            for (name, value) in lines {
                row! {
                    text(name.as_str()).caption().muted().width(120.0);
                    text(value.as_str()).caption().grow(1.0);
                }
                .gap(Space::NORMAL);
            }
        }
        .gap(Space::SNUG)
        .build(),
        Dialog::ConfirmDelete { items } => column! {
            text(tr!("files.dialog.confirm-delete")).semibold();
            text(tr!("files.dialog.confirm-delete-detail", count = items.len())).caption();
        }
        .gap(Space::NORMAL)
        .build(),
        Dialog::ConfirmRun { program } => column! {
            text(tr!("files.dialog.confirm-run")).semibold();
            text(tr!("files.dialog.confirm-run-detail", name = &program.name())).caption();
        }
        .gap(Space::NORMAL)
        .build(),
        Dialog::Password {
            asking,
            typed,
            wrong,
        } => password_body(asking, typed, *wrong).build(),
        Dialog::Compress(sheet) => packing_body(sheet).build(),
        Dialog::Convert(sheet) => converting_body(sheet).build(),
    };

    let buttons = match dialog {
        Dialog::Properties { .. } => row! {
            spacer().grow(1.0);
            button(text(tr!("files.dialog.close")))
                .primary()
                .on_press(WindowState::dismiss);
        }
        .gap(Space::SNUG)
        .build(),
        Dialog::ConfirmDelete { .. } => row! {
            spacer().grow(1.0);
            button(text(tr!("files.dialog.cancel")))
                .ghost()
                .on_press(WindowState::dismiss);
            button(text(tr!("files.dialog.delete")))
                .danger()
                .on_press(WindowState::accept);
        }
        .gap(Space::SNUG)
        .build(),
        Dialog::ConfirmRun { .. } => choice(tr!("files.dialog.run"), false).build(),
        Dialog::Password { asking, .. } => choice(
            if asking.is_volume() {
                tr!("files.unlock.open")
            } else {
                tr!("files.locked.open")
            },
            false,
        )
        .build(),
        Dialog::Compress(sheet) => choice(
            tr!("files.compress.go"),
            sheet.name.text().trim().is_empty(),
        )
        .build(),
        Dialog::Convert(_) => choice(tr!("files.convert.go"), false).build(),
    };

    frame::sheet(
        440.0,
        (DIALOG_KEY, key),
        column! {
            body;
            buttons;
        }
        .gap(Space::ROOMY)
        .padding(20.0),
        WindowState::dismiss,
    )
}

fn packing_body(sheet: &Compressing) -> impl View {
    let format = files_archive::OFFERS[sheet.format];
    column! {
        text(tr!("files.compress.heading", count = sheet.sources.len())).semibold();
        row! {
            text_field(&sheet.name)
                .placeholder(tr!("files.compress.name"))
                .autofocus(true)
                .on_change(|| {})
                .on_submit(WindowState::accept)
                .grow(1.0);
            combo(sheet.format)
                .options(
                    files_archive::OFFERS
                        .iter()
                        .map(|offered| format!(".{}", offered.extension())),
                )
                .on_select(|window: &mut WindowState, index| window.compress_format(index))
                .width(140.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        row! {
            text(tr!("files.compress.level")).caption().muted().width(96.0);
            segmented(sheet.compression)
                .segments([
                    tr!("files.compress.level-default"),
                    tr!("files.compress.level-fastest"),
                    tr!("files.compress.level-best"),
                ])
                .on_select(|window: &mut WindowState, index| window.compress_level(index))
                .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        if format.supports_encryption() {
            text_field(&sheet.password)
                .placeholder(tr!("files.compress.password"))
                .secret(true)
                .fill_width();
        }
    }
    .gap(Space::NORMAL)
}

fn converting_body(sheet: &Converting) -> impl View {
    let target = sheet.chosen();
    let sizes = target.sizes();
    column! {
        text(tr!("files.convert.heading", count = sheet.sources.len())).semibold();
        row! {
            text(tr!("files.convert.format")).caption().muted().width(96.0);
            combo(sheet.target)
                .options(sheet.targets.iter().map(|target| wording::conversion_choice(*target)))
                .on_select(|window: &mut WindowState, index| window.convert_format(index))
                .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        if target.is_lossy() {
            row! {
                text(tr!("files.convert.quality")).caption().muted().width(96.0);
                segmented(sheet.quality)
                    .segments([
                        tr!("files.convert.quality-standard"),
                        tr!("files.convert.quality-high"),
                        tr!("files.convert.quality-best"),
                    ])
                    .on_select(|window: &mut WindowState, index| window.convert_quality(index))
                    .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        if !sizes.is_empty() {
            row! {
                text(tr!("files.convert.size")).caption().muted().width(96.0);
                combo(sheet.size)
                    .options(sizes.iter().map(|scale| wording::scale(*scale)))
                    .on_select(|window: &mut WindowState, index| window.convert_size(index))
                    .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
    }
    .gap(Space::NORMAL)
}

fn password_body(asking: &Asking, typed: &TextDocument, wrong: bool) -> impl View {
    let words = |volume: &'static str, file: &'static str| {
        if asking.is_volume() { volume } else { file }
    };
    column! {
        text(tr!(
            words("files.unlock.heading", "files.locked.heading"),
            name = &asking.name(),
        ))
        .semibold();
        text(Label::translated(words("files.unlock.detail", "files.locked.detail")))
            .caption()
            .muted();
        text_field(typed)
            .placeholder(Label::translated(words(
                "files.unlock.passphrase",
                "files.locked.password",
            )))
            .secret(true)
            .autofocus(true)
            .error(wrong)
            .on_submit(WindowState::accept)
            .fill_width();
        if wrong {
            text(Label::translated(words("files.unlock.wrong", "files.locked.wrong")))
                .caption()
                .color(TextColor::Danger);
        }
    }
    .gap(Space::NORMAL)
}

fn choice(accept: Label, blocked: bool) -> impl View {
    row! {
        spacer().grow(1.0);
        button(text(tr!("files.dialog.cancel")))
            .ghost()
            .on_press(WindowState::dismiss);
        button(text(accept))
            .primary()
            .enabled(!blocked)
            .on_press(WindowState::accept);
    }
    .gap(Space::SNUG)
}

fn collision(question: &files_vfs::Conflict) -> Vec<Element> {
    let describe = |entry: &files_core::Entry| {
        tr!(
            "files.conflict.detail",
            size = &format_bytes(entry.size),
            modified = &files_core::format::timestamp(entry.modified),
        )
    };
    frame::sheet(
        520.0,
        CONFLICT_KEY,
        column! {
            text(tr!("files.conflict.exists", name = &question.source.name)).semibold();
            column! {
                row! {
                    text(tr!("files.conflict.source")).caption().muted().width(110.0);
                    text(describe(&question.source)).caption();
                }
                .gap(Space::SNUG);
                row! {
                    text(tr!("files.conflict.existing")).caption().muted().width(110.0);
                    text(describe(&question.existing)).caption();
                }
                .gap(Space::SNUG);
            }
            .gap(Space::TIGHT);
            row! {
                button(text(tr!("files.conflict.skip-all")))
                    .ghost()
                    .small()
                    .on_press(|jobs: &mut Jobs| jobs.resolve(Resolution::Skip, true));
                spacer().grow(1.0);
                button(text(tr!("files.conflict.skip")))
                    .ghost()
                    .on_press(|jobs: &mut Jobs| jobs.resolve(Resolution::Skip, false));
                button(text(tr!("files.conflict.keep")))
                    .secondary()
                    .on_press(|jobs: &mut Jobs| jobs.resolve(Resolution::Keep, false));
                button(text(tr!("files.conflict.replace")))
                    .primary()
                    .on_press(|jobs: &mut Jobs| jobs.resolve(Resolution::Replace, false));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::ROOMY)
        .padding(20.0),
        |jobs: &mut Jobs| jobs.cancel(),
    )
}
