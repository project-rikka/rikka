use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::command::Command;
use crate::cut::Cut;
use crate::timeline::scale::Scale;
use crate::trimmer::Trimmer;

const GLYPH: f32 = 18.0;
const ZOOM_WIDTH: f32 = 120.0;

pub(crate) fn bar(cut: &Cut) -> impl View {
    let current = cut.trimmer();
    toolbar()
        .divided()
        .gap(Space::TIGHT)
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .group(tool_group([zoom(cut.scale().clone())]).priority(ToolPriority::Low))
        .group(tool_group([
            Command::Import.tool(MaterialSymbol::LIBRARY_ADD, cut)
        ]))
        .group(
            tool_group([
                Command::Undo.tool(MaterialSymbol::UNDO, cut),
                Command::Redo.tool(MaterialSymbol::REDO, cut),
            ])
            .priority(ToolPriority::High),
        )
        .group(
            tool_group([
                Command::Cut.tool(MaterialSymbol::CONTENT_CUT, cut),
                Command::Copy.tool(MaterialSymbol::CONTENT_COPY, cut),
                Command::Paste.tool(MaterialSymbol::CONTENT_PASTE, cut),
                Command::Split.tool(MaterialSymbol::VERTICAL_SPLIT, cut),
                Command::Delete.tool(MaterialSymbol::DELETE, cut),
            ])
            .priority(ToolPriority::High),
        )
        .group(tool_group([Command::Snapping
            .tool(MaterialSymbol::ALIGN_HORIZONTAL_CENTER, cut)
            .checked(cut.snapping())]))
        .group(tool_group(Trimmer::ALL.map(|trimmer| {
            Command::Trimmer(trimmer)
                .tool(trimmer.symbol(), cut)
                .checked(current == trimmer)
        })))
        .group(tool_group([
            Command::AddMarker.tool(MaterialSymbol::BOOKMARK_ADD, cut)
        ]))
        .group(
            tool_group([
                Command::New.tool(MaterialSymbol::NOTE_ADD, cut),
                Command::Open.tool(MaterialSymbol::FOLDER_OPEN, cut),
                Command::Save.tool(MaterialSymbol::SAVE, cut),
            ])
            .priority(ToolPriority::Low),
        )
        .background(palette().surface)
}

fn zoom(scale: Scale) -> ToolItem {
    let level = scale.level();
    let zoom_to = |cut: &mut Cut, level: f32| cut.zoom_to(level);
    let control = row! {
        icon(MaterialSymbol::ZOOM_IN)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        live_observed(move || {
            slider(scale.level())
                .range(0.0..=1.0)
                .on_change(zoom_to)
                .width(ZOOM_WIDTH)
        });
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center);
    tool_item(control).overflow([menu_slider(tr!("cut.timeline.zoom"), level, 0.0..=1.0)
        .icon(MaterialSymbol::ZOOM_IN)
        .on_change(zoom_to)])
}
