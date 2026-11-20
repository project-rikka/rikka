use std::collections::HashSet;
use std::ops::Range;

use files_core::{Location, SortKey};
use files_dialog::{Choice, Kind, OFF, ON};
use rikka_app::prelude::*;
use rikka_ui::{Element, View};
use rikka_widgets::column;

use crate::chooser::dialog::Dialog;
use crate::frame;
use crate::places::Places;
use crate::tab::Tab;
use crate::window::keys;

const QUESTION_KEY: &str = "chooser-question";

pub(crate) fn view(dialog: &Dialog) -> Element {
    frame::stage(
        column! {
            bar(dialog);
            rule();
            row! {
                sidebar(dialog);
                listing(dialog);
            }
            .grow(1.0);
            footer(dialog);
            if let Some(at) = dialog.overwrite.showing() {
                for layer in question(at, dialog.overwrite.key()) {
                    layer;
                }
            }
        }
        .fill(),
    )
}

pub(crate) fn bind(window: Window<Dialog>) -> Window<Dialog> {
    window
        .on_command(Command::Back, Dialog::back)
        .on_command(Command::Forward, Dialog::forward)
        .on_command(Command::Up, Dialog::up)
        .on_command(Command::Home, Dialog::home)
        .on_command(Command::Refresh, Dialog::refresh)
        .on_command(Command::Rename, |tab: &mut Tab| tab.begin_rename())
        .on_command(Command::SelectAll, Dialog::select_all)
        .on_chord(keys::LOCATION[0].0, keys::LOCATION[0].1, edit_location)
        .on_chord(keys::LOCATION[1].0, keys::LOCATION[1].1, edit_location)
        .on_chord(keys::HIDDEN.0, keys::HIDDEN.1, Dialog::toggle_hidden)
        .on_chord(keys::NEW_FOLDER.0, keys::NEW_FOLDER.1, Dialog::make_folder)
        .on_chord(keys::BACK.0, keys::BACK.1, Dialog::back)
        .on_chord(Key::Escape, Modifiers::NONE, Dialog::escape)
}

fn edit_location(tab: &mut Tab) {
    tab.edit_path(true);
}

fn bar(dialog: &Dialog) -> impl View {
    let tab = &dialog.tab;
    let can_go_up = dialog.services.vfs.parent_of(tab.at()).is_some();
    let navigation = tool_group([
        frame::step(
            MaterialSymbol::ARROW_BACK,
            Command::Back,
            Dialog::back,
            tab.history().can_go_back(),
        ),
        frame::step(
            MaterialSymbol::ARROW_FORWARD,
            Command::Forward,
            Dialog::forward,
            tab.history().can_go_forward(),
        ),
        frame::step(
            MaterialSymbol::ARROW_UPWARD,
            Command::Up,
            Dialog::up,
            can_go_up,
        ),
    ])
    .gap(Space::HAIR)
    .priority(ToolPriority::Pinned);
    let mut actions = Vec::new();
    if dialog.ask.kind != Kind::Open && tab.folder_allows().create {
        actions.push(
            tool_button(
                MaterialSymbol::CREATE_NEW_FOLDER,
                tr!("files.menu.new-folder"),
            )
            .control_size(ControlSize::Medium)
            .on_press(Dialog::make_folder)
            .into(),
        );
    }
    let hidden = match tab.shows_hidden() {
        true => MaterialSymbol::VISIBILITY,
        false => MaterialSymbol::VISIBILITY_OFF,
    };
    actions.push(
        tool_item(
            tool_button(hidden, tr!("files.menu.hidden"))
                .control_size(ControlSize::Medium)
                .on_press(Dialog::toggle_hidden),
        )
        .overflow([menu_item(tr!("files.menu.hidden"))
            .icon(hidden)
            .selected(tab.shows_hidden())
            .on_press(Dialog::toggle_hidden)]),
    );
    toolbar()
        .gap(Space::SNUG)
        .padding(Inset::xy(10.0, 8.0))
        .control_size(ControlSize::Medium)
        .group(navigation)
        .flexible(
            frame::path_bar(
                tab.at(),
                tab.editing_path(),
                |dialog: &mut Dialog, at: Location| dialog.go(at),
                |tab: &mut Tab| tab.edit_path(true),
                |tab: &mut Tab| tab.edit_path(false),
                Dialog::path,
            ),
            frame::PATH_MIN,
        )
        .group(tool_group(actions))
}

fn sidebar(dialog: &Dialog) -> impl View {
    frame::places(
        Places::new(dialog.services.rows(), Some(dialog.tab.at()))
            .managed(false)
            .on_open(|dialog: &mut Dialog, index| dialog.open_place(index))
            .on_open_tab(|dialog: &mut Dialog, index| dialog.open_place(index)),
    )
}

fn listing(dialog: &Dialog) -> impl View {
    let tab = &dialog.tab;
    frame::listing(
        tab,
        0,
        tab.browser()
            .on_open(|dialog: &mut Dialog, index| dialog.open(index))
            .on_select(
                |dialog: &mut Dialog, chosen: HashSet<usize>, focus: Option<usize>| {
                    dialog.select(chosen, focus);
                },
            )
            .on_sort(|dialog: &mut Dialog, key: SortKey| dialog.sort(key))
            .on_visible(|dialog: &mut Dialog, range: Range<usize>| dialog.visible(range))
            .on_rename(|dialog: &mut Dialog, name: String| dialog.rename_to(&name))
            .on_rename_cancel(Tab::end_rename),
    )
}

fn footer(dialog: &Dialog) -> impl View {
    let accept = match dialog.ask.accept_label.as_str() {
        "" => match dialog.ask.kind {
            Kind::Open => tr!("files.chooser.open"),
            Kind::Save | Kind::SaveInto => tr!("files.chooser.save"),
        },
        label => Label::from(label),
    };
    column! {
        rule();
        column! {
            if let Some(failure) = &dialog.tab.failure {
                banner(failure.as_str())
                    .on_dismiss(tr!("files.job.dismiss"), Dialog::dismiss)
                    .fill_width();
            }
            match dialog.ask.kind {
                Kind::Save => row! {
                    text(tr!("files.chooser.name")).label().muted();
                    text_field(&dialog.name)
                        .placeholder(tr!("files.chooser.name"))
                        .on_change(|| {})
                        .on_submit(Dialog::accept)
                        .grow(1.0);
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
                Kind::SaveInto => {
                    text(tr!("files.chooser.saving-here", count = dialog.ask.names.len())).label()
                }
                Kind::Open => Element::empty(),
            }
            row! {
                for (index, choice) in dialog.ask.choices.iter().enumerate() {
                    choice_view(dialog, index, choice);
                }
                spacer().grow(1.0);
                if !dialog.ask.filters.is_empty() {
                    combo(dialog.filter)
                        .options(dialog.ask.filters.iter().map(|filter| filter.label.as_str()))
                        .placeholder(tr!("files.chooser.filter"))
                        .on_select(|dialog: &mut Dialog, index| dialog.choose_filter(index))
                        .width(220.0);
                }
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center);
            row! {
                spacer().grow(1.0);
                button(text(tr!("files.dialog.cancel")))
                    .ghost()
                    .on_press(Dialog::cancel);
                button(text(accept))
                    .primary()
                    .enabled(can_accept(dialog))
                    .on_press(Dialog::accept);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::SNUG)
        .padding(Inset::xy(12.0, 10.0))
        .fill_width();
    }
}

fn choice_view(dialog: &Dialog, index: usize, choice: &Choice) -> Element {
    let picked = dialog.picks.get(index).cloned().unwrap_or_default();
    if choice.is_switch() {
        return checkbox(picked == ON)
            .label(text(choice.label.as_str()).label())
            .on_toggle(move |dialog: &mut Dialog, on: bool| {
                dialog.pick(index, if on { ON } else { OFF }.to_owned());
            })
            .build();
    }
    let ids: Vec<String> = choice.options.iter().map(|one| one.id.clone()).collect();
    row! {
        text(choice.label.as_str()).label();
        combo(choice.options.iter().position(|one| one.id == picked))
            .options(choice.options.iter().map(|one| one.label.as_str()))
            .on_select(move |dialog: &mut Dialog, at: usize| {
                dialog.pick(index, ids.get(at).cloned().unwrap_or_default());
            })
            .width(180.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .build()
}

fn can_accept(dialog: &Dialog) -> bool {
    match dialog.ask.kind {
        Kind::Open => dialog.ask.directory || dialog.tab.chosen_count() > 0,
        Kind::Save => !dialog.name.text().trim().is_empty(),
        Kind::SaveInto => dialog.tab.folder_allows().create,
    }
}

fn question(at: &Location, key: u64) -> Vec<Element> {
    frame::sheet(
        440.0,
        (QUESTION_KEY, key),
        column! {
            column! {
                text(tr!("files.chooser.replace", name = &at.name())).semibold();
                text(tr!("files.chooser.replace-detail")).caption();
            }
            .gap(Space::NORMAL);
            row! {
                spacer().grow(1.0);
                button(text(tr!("files.dialog.cancel")))
                    .ghost()
                    .on_press(Dialog::dismiss);
                button(text(tr!("files.chooser.replace-yes")))
                    .danger()
                    .on_press(Dialog::overwrite);
            }
            .gap(Space::SNUG);
        }
        .gap(Space::ROOMY)
        .padding(20.0),
        Dialog::dismiss,
    )
}
