use rikka_app::prelude::*;
use rikka_widgets::row;

use paint_document::StackOrder;
use rikka_image_edit::{Flip, QuarterTurn};

use crate::command::{Command, Objecting};
use crate::paint::Paint;
use crate::session::{Kind, Modification};
use crate::tab::{Layering, Tab};
use crate::{develop_menu, tools, window};

const GLYPH: f32 = 18.0;
const TABS: &str = "rikka.paint.tab";

pub(crate) fn bar(paint: &Paint) -> Option<CaptionBar> {
    let leading = row! {
        icon(MaterialSymbol::PALETTE)
            .extent(Length::px(GLYPH))
            .color(palette().accent);
        menus(paint);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);
    let bar = caption().leading(leading);
    let bar = match strip(paint) {
        Some(strip) => bar.fill(strip),
        None => bar,
    };
    Some(bar.into())
}

fn menus(paint: &Paint) -> MenuBar {
    let entry = |command: Command, label: Label| command.entry(label, paint);
    menu_bar()
        .menu(
            tr!("paint.menu.file"),
            [
                entry(Command::NewImage, tr!("paint.command.new_image")),
                entry(Command::Open, tr!("paint.command.open")),
                menu_separator(),
                entry(Command::Save, tr!("paint.command.save")),
                entry(Command::SaveAs, tr!("paint.command.save_as")),
                menu_separator(),
                entry(Command::CloseTab, tr!("paint.command.close_tab")),
                entry(Command::Quit, tr!("paint.command.quit")),
            ],
        )
        .menu(
            tr!("paint.menu.edit"),
            [
                entry(Command::Undo, tr!("paint.command.undo")),
                entry(Command::Redo, tr!("paint.command.redo")),
                menu_separator(),
                entry(Command::Object(Objecting::Cut), tr!("paint.command.cut")),
                entry(Command::Object(Objecting::Copy), tr!("paint.command.copy")),
                entry(
                    Command::Object(Objecting::Paste),
                    tr!("paint.command.paste"),
                ),
                entry(
                    Command::Object(Objecting::Duplicate),
                    tr!("paint.command.duplicate"),
                ),
                entry(
                    Command::Object(Objecting::Delete),
                    tr!("paint.command.delete"),
                ),
                menu_item(tr!("paint.command.arrange")).items(arrange_items(paint)),
                menu_separator(),
                entry(Command::SelectAll, tr!("paint.command.select_all")),
                entry(Command::Deselect, tr!("paint.command.deselect")),
                entry(
                    Command::InvertSelection,
                    tr!("paint.command.invert_selection"),
                ),
                entry(
                    Command::SelectLayerPixels,
                    tr!("paint.command.select_layer_pixels"),
                ),
                menu_item(tr!("paint.command.modify")).items([
                    entry(
                        Command::Begin(Kind::Modify(Modification::Border)),
                        tr!("paint.command.border"),
                    ),
                    entry(
                        Command::Begin(Kind::Modify(Modification::Smooth)),
                        tr!("paint.command.smooth"),
                    ),
                    entry(
                        Command::Begin(Kind::Modify(Modification::Expand)),
                        tr!("paint.command.expand"),
                    ),
                    entry(
                        Command::Begin(Kind::Modify(Modification::Contract)),
                        tr!("paint.command.contract"),
                    ),
                    entry(
                        Command::Begin(Kind::Modify(Modification::Feather)),
                        tr!("paint.command.feather"),
                    ),
                ]),
                menu_separator(),
                entry(
                    Command::EraseSelection,
                    tr!("paint.command.erase_selection"),
                ),
                entry(Command::FillSelection, tr!("paint.command.fill_selection")),
            ],
        )
        .menu(
            tr!("paint.menu.view"),
            [
                entry(Command::ZoomIn, tr!("paint.command.zoom_in")),
                entry(Command::ZoomOut, tr!("paint.command.zoom_out")),
                menu_separator(),
                entry(Command::Fit, tr!("paint.command.fit")),
                entry(Command::ActualSize, tr!("paint.command.actual_size")),
                menu_separator(),
                entry(Command::NextTab, tr!("paint.command.next_tab")),
                entry(Command::PreviousTab, tr!("paint.command.previous_tab")),
                menu_separator(),
                entry(Command::Assistant, tr!("paint.command.assistant")),
            ],
        )
        .menu(
            tr!("paint.menu.image"),
            [
                entry(
                    Command::CropToSelection,
                    tr!("paint.command.crop_to_selection"),
                ),
                menu_separator(),
                entry(Command::Begin(Kind::Resize), tr!("paint.command.resize")),
                entry(
                    Command::Begin(Kind::CanvasSize),
                    tr!("paint.command.canvas_size"),
                ),
                menu_item(tr!("paint.command.rotation")).items([
                    entry(
                        Command::Turn(QuarterTurn::Half),
                        tr!("paint.operation.turn_half"),
                    ),
                    entry(
                        Command::Turn(QuarterTurn::Clockwise),
                        tr!("paint.operation.turn_clockwise"),
                    ),
                    entry(
                        Command::Turn(QuarterTurn::CounterClockwise),
                        tr!("paint.operation.turn_counter_clockwise"),
                    ),
                    entry(Command::Begin(Kind::Rotate), tr!("paint.command.rotate")),
                    menu_separator(),
                    entry(
                        Command::Flip(Flip::Horizontal),
                        tr!("paint.operation.flip_horizontal"),
                    ),
                    entry(
                        Command::Flip(Flip::Vertical),
                        tr!("paint.operation.flip_vertical"),
                    ),
                ]),
                menu_separator(),
                entry(
                    Command::Begin(Kind::ConvertProfile),
                    tr!("paint.command.convert_profile"),
                ),
            ],
        )
        .menu(tr!("paint.menu.layers"), layers_menu(paint))
        .menu(
            tr!("paint.menu.adjustments"),
            develop_menu::adjustments(paint),
        )
        .menu(tr!("paint.menu.effects"), develop_menu::effects(paint))
        .menu(tr!("paint.menu.tools"), tools::menu(paint))
}

pub(crate) fn arrange_items(paint: &Paint) -> Vec<MenuItem> {
    let entry =
        |to: StackOrder, label: Label| Command::Object(Objecting::Stack(to)).entry(label, paint);
    vec![
        entry(StackOrder::Front, tr!("paint.command.bring_to_front")),
        entry(StackOrder::Forward, tr!("paint.command.bring_forward")),
        entry(StackOrder::Backward, tr!("paint.command.send_backward")),
        entry(StackOrder::Back, tr!("paint.command.send_to_back")),
    ]
}

fn layers_menu(paint: &Paint) -> Vec<MenuItem> {
    let entry = |command: Command, label: Label| command.entry(label, paint);
    vec![
        entry(
            Command::Layer(Layering::New),
            tr!("paint.command.new_layer"),
        ),
        entry(
            Command::Layer(Layering::Duplicate),
            tr!("paint.command.duplicate_layer"),
        ),
        entry(
            Command::Layer(Layering::Delete),
            tr!("paint.command.delete_layer"),
        ),
        menu_separator(),
        entry(
            Command::Begin(Kind::LayerProperties),
            tr!("paint.command.layer_properties"),
        ),
        entry(
            Command::Layer(Layering::FromBackground),
            tr!("paint.command.layer_from_background"),
        ),
        entry(
            Command::Object(Objecting::Rasterize),
            tr!("paint.command.rasterize_objects"),
        ),
        menu_separator(),
        entry(
            Command::Layer(Layering::Forward),
            tr!("paint.command.bring_forward"),
        ),
        entry(
            Command::Layer(Layering::Backward),
            tr!("paint.command.send_backward"),
        ),
        menu_separator(),
        entry(
            Command::Layer(Layering::MergeDown),
            tr!("paint.command.merge_down"),
        ),
        entry(
            Command::Layer(Layering::MergeVisible),
            tr!("paint.command.merge_visible"),
        ),
        entry(
            Command::Layer(Layering::Flatten),
            tr!("paint.command.flatten"),
        ),
    ]
}

fn strip(paint: &Paint) -> Option<Element> {
    if paint.tabs().is_empty() {
        return None;
    }
    Some(
        tabs(paint.active_index())
            .items(paint.tabs().iter().map(strip_tab))
            .in_caption()
            .on_select(|paint: &mut Paint, index| paint.select(index))
            .on_close(|paint: &mut Paint, index| paint.close(index))
            .on_new(Paint::show_open_dialog)
            .transfers(
                Transfer::group(TABS)
                    .take(|paint: &mut Paint, index| paint.lift(index))
                    .insert(|paint: &mut Paint, at, tab| paint.land(at, tab))
                    .tear_off(
                        |_paint: &mut Paint, tab: Tab| open_window(window::holding(tab)),
                        false,
                    ),
            )
            .on_finished(Paint::settled)
            .build(),
    )
}

fn strip_tab(tab: &Tab) -> rikka_widgets::Tab {
    rikka_widgets::tab(tab.title()).icon(MaterialSymbol::IMAGE)
}
