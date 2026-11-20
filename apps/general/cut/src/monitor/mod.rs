mod seekbar;
mod tools;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::command::Command;
use crate::cut::Cut;
use crate::stage::Stage;
use crate::wording;

use seekbar::Seekbar;

const GLYPH: f32 = 18.0;
const VOLUME: f32 = 88.0;
const SEEKBAR_LEAST: f32 = 80.0;

pub(crate) fn monitor(cut: &Cut) -> impl View {
    column! {
        tools::tools(cut);
        Stage {
            preview: cut.playback().preview().clone(),
            quality: cut.quality(),
            project: cut.project().clone(),
            transport: cut.transport().clone(),
            selected: cut.selected().clone(),
            tool: cut.tool(),
            draft: cut.draft().cloned(),
            live: cut.live().clone(),
            snapping: cut.snapping(),
            sighting: Default::default(),
        }
        .build()
        .grow(1.0)
        .fill_width();
        transport(cut);
    }
    .fill()
}

fn transport(cut: &Cut) -> impl View {
    let playing = cut.transport().clone();
    let play = live_observed(move || {
        let symbol = match playing.is_playing() {
            true => MaterialSymbol::PAUSE,
            false => MaterialSymbol::PLAY_ARROW,
        };
        tool_button(symbol, wording::command(Command::PlayPause))
            .shortcut(Command::PlayPause.chord())
            .on_press(|cut: &mut Cut| Command::PlayPause.run(cut))
    });
    let seekbar = Seekbar {
        transport: cut.transport().clone(),
        end: cut.project().end(),
    }
    .build()
    .grow(1.0);
    toolbar()
        .divided()
        .gap(Space::TIGHT)
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .group(
            tool_group([
                tool_item(play),
                Command::Stop.tool(MaterialSymbol::STOP, cut).into(),
            ])
            .priority(ToolPriority::Pinned),
        )
        .group(
            tool_group([
                Command::GoToStart.tool(MaterialSymbol::FIRST_PAGE, cut),
                Command::PreviousEdit.tool(MaterialSymbol::SKIP_PREVIOUS, cut),
                Command::Step(-1).tool(MaterialSymbol::CHEVRON_LEFT, cut),
            ])
            .priority(ToolPriority::High),
        )
        .flexible(seekbar, SEEKBAR_LEAST)
        .group(
            tool_group([
                Command::Step(1).tool(MaterialSymbol::CHEVRON_RIGHT, cut),
                Command::NextEdit.tool(MaterialSymbol::SKIP_NEXT, cut),
                Command::GoToEnd.tool(MaterialSymbol::LAST_PAGE, cut),
            ])
            .priority(ToolPriority::High),
        )
        .group(tool_group([volume(cut.volume())]).priority(ToolPriority::Low))
        .background(palette().surface)
}

fn volume(volume: f32) -> ToolItem {
    let symbol = match volume > 0.0 {
        true => MaterialSymbol::VOLUME_UP,
        false => MaterialSymbol::VOLUME_OFF,
    };
    let change = |cut: &mut Cut, volume: f32| cut.set_volume(volume);
    let control = row! {
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        slider(volume)
            .range(0.0..=1.0)
            .on_change(change)
            .width(VOLUME);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center);
    tool_item(control).overflow([menu_slider(tr!("cut.monitor.volume"), volume, 0.0..=1.0)
        .icon(symbol)
        .on_change(change)])
}
