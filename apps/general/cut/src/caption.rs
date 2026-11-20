use cut_filmstrip::Mode;
use cut_project::{Height, TransitionFamily};
use cut_waveform::Scale;
use rikka_app::prelude::*;
use rikka_effects::Family;
use rikka_widgets::row;

use crate::command::Command;
use crate::cut::{Cut, EditDirection};
use crate::quality::Quality;
use crate::tool::Tool;
use crate::transport::Direction;
use crate::trimmer::Trimmer;
use crate::wording;

const GLYPH: f32 = 18.0;

pub(crate) fn bar(cut: &Cut) -> Option<CaptionBar> {
    let leading = row! {
        icon(MaterialSymbol::MOVIE_EDIT)
            .extent(Length::px(GLYPH))
            .color(palette().accent);
        menus(cut);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);
    Some(caption().leading(leading).into())
}

fn menus(cut: &Cut) -> MenuBar {
    let entry = |command: Command| command.entry(cut);
    let undo = match cut.document().undoable() {
        Some(action) => tr!(
            "cut.command.undo_action",
            action = &wording::action(action).to_string()
        ),
        None => wording::command(Command::Undo),
    };
    let redo = match cut.document().redoable() {
        Some(action) => tr!(
            "cut.command.redo_action",
            action = &wording::action(action).to_string()
        ),
        None => wording::command(Command::Redo),
    };
    menu_bar()
        .menu(
            tr!("cut.menu.file"),
            [
                entry(Command::New),
                entry(Command::Open),
                menu_separator(),
                entry(Command::Import),
                entry(Command::RelinkMedia),
                menu_separator(),
                entry(Command::Save),
                entry(Command::SaveAs),
                menu_separator(),
                entry(Command::ProjectSettings),
                menu_separator(),
                entry(Command::Export),
                entry(Command::ExportFrame),
                menu_separator(),
                entry(Command::Quit),
            ],
        )
        .menu(
            tr!("cut.menu.edit"),
            [
                Command::Undo.entry_named(undo, cut),
                Command::Redo.entry_named(redo, cut),
                menu_separator(),
                entry(Command::Cut),
                entry(Command::Copy),
                entry(Command::Paste),
                entry(Command::PasteAttributes),
                entry(Command::PasteEffects),
                entry(Command::Delete),
                entry(Command::RippleDelete),
                entry(Command::RemoveEffects),
                menu_separator(),
                entry(Command::ReplaceMedia),
                menu_separator(),
                entry(Command::Split),
                trimming(cut),
                menu_separator(),
                entry(Command::SelectAll),
                entry(Command::Deselect),
                menu_separator(),
                entry(Command::Snapping).selected(cut.snapping()),
            ],
        )
        .menu(tr!("cut.menu.insert"), inserting(cut))
        .menu(
            tr!("cut.menu.playback"),
            [
                entry(Command::PlayPause),
                entry(Command::Stop),
                entry(Command::Shuttle(Direction::Backward)),
                entry(Command::Pause),
                entry(Command::Shuttle(Direction::Forward)),
                menu_separator(),
                entry(Command::Step(-1)),
                entry(Command::Step(1)),
                entry(Command::PreviousEdit),
                entry(Command::NextEdit),
                entry(Command::PreviousKey),
                entry(Command::NextKey),
                entry(Command::GoToStart),
                entry(Command::GoToEnd),
                menu_separator(),
                entry(Command::MarkIn),
                entry(Command::MarkOut),
                entry(Command::GoToIn),
                entry(Command::GoToOut),
                entry(Command::ClearMarks),
            ],
        )
        .menu(
            tr!("cut.menu.markers"),
            [
                entry(Command::AddMarker),
                entry(Command::AddClipMarker),
                menu_separator(),
                entry(Command::PreviousMarker),
                entry(Command::NextMarker),
                menu_separator(),
                entry(Command::EditMarker),
                entry(Command::DeleteMarker),
                entry(Command::ClearMarkers),
            ],
        )
        .menu(tr!("cut.menu.view"), viewing(cut))
}

fn trimming(cut: &Cut) -> MenuItem {
    let mut items: Vec<MenuItem> = Trimmer::ALL
        .map(|trimmer| {
            Command::Trimmer(trimmer)
                .entry(cut)
                .selected(cut.trimmer() == trimmer)
        })
        .into();
    items.extend([
        menu_separator(),
        Command::RippleTrimTo(EditDirection::Previous).entry(cut),
        Command::RippleTrimTo(EditDirection::Next).entry(cut),
        Command::TrimTo(EditDirection::Previous).entry(cut),
        Command::TrimTo(EditDirection::Next).entry(cut),
        Command::ExtendEdit.entry(cut),
    ]);
    menu_item(tr!("cut.menu.trim")).items(items)
}

fn inserting(cut: &Cut) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = Tool::made()
        .map(|tool| Command::Add(tool).entry(cut))
        .collect();
    items.push(menu_separator());
    items.push(Command::AddRange.entry(cut));
    items.push(Command::AddWhole.entry(cut));
    items.push(Command::AddCamera.entry(cut));
    items.push(menu_separator());
    items.push(effect_menu(cut));
    items.push(transition_menu(cut));
    items
}

pub(crate) fn transition_menu(cut: &Cut) -> MenuItem {
    let mut families: Vec<MenuItem> = TransitionFamily::ALL
        .into_iter()
        .map(|family| {
            menu_item(wording::transition_family(family)).items(family.kinds().map(|kind| {
                menu_item(wording::transition_kind(kind))
                    .on_press(move |cut: &mut Cut| cut.transit(kind))
            }))
        })
        .collect();
    if cut.has_removable_transition() {
        families.extend([
            menu_separator(),
            menu_item(tr!("cut.transition.remove")).on_press(Cut::remove_transition),
        ]);
    }
    menu_item(tr!("cut.transition.apply"))
        .enabled(!cut.transition_targets().is_empty())
        .items(families)
}

pub(crate) fn effect_menu(cut: &Cut) -> MenuItem {
    menu_item(tr!("cut.menu.effects"))
        .enabled(cut.selected_visual().is_some())
        .items(Family::ALL.map(|family| {
            menu_item(family.name()).items(
                family
                    .kinds()
                    .map(|kind| Command::AddEffect(kind).entry(cut)),
            )
        }))
}

fn viewing(cut: &Cut) -> Vec<MenuItem> {
    let mut items = vec![
        Command::ZoomIn.entry(cut),
        Command::ZoomOut.entry(cut),
        Command::ZoomToFit.entry(cut),
        menu_separator(),
        Command::KeyLanes
            .entry(cut)
            .selected(cut.keyframing().is_open()),
        Command::GraphEditor
            .entry(cut)
            .selected(cut.keyframing().is_graph()),
        menu_separator(),
        menu_header(tr!("cut.menu.quality")),
    ];
    items.extend(Quality::ALL.map(|quality| {
        Command::Quality(quality)
            .entry(cut)
            .selected(cut.quality() == quality)
    }));
    let display = cut.display();
    let height = cut.project().layer_height();
    items.extend([
        menu_separator(),
        menu_item(tr!("cut.menu.thumbnails")).items(Mode::ALL.map(|mode| {
            Command::Thumbnails(mode)
                .entry(cut)
                .selected(display.thumbnails == mode)
        })),
        menu_item(tr!("cut.menu.waveform")).items(Scale::ALL.map(|scale| {
            Command::Waveform(scale)
                .entry(cut)
                .selected(display.waveform == scale)
        })),
        menu_item(tr!("cut.menu.layer_height")).items(Height::PRESETS.map(|preset| {
            Command::LayerHeight(preset)
                .entry(cut)
                .selected(height == preset)
        })),
    ]);
    items
}
