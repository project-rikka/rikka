use cut_project::{Clip, KeyRef};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::names::{Name, Names};
use super::outline::{Outline, SUMMARY};
use crate::command::Command;
use crate::controls::{glyph_button, toggle};
use crate::cut::{Cut, Plot};
use crate::wording;

const GLYPH: f32 = 16.0;

fn names(clip: &Clip, outline: &Outline) -> Vec<Name> {
    outline
        .properties
        .iter()
        .map(|property| Name {
            property: property.clone(),
            label: wording::property(property, clip).to_string(),
            keys: clip
                .channel(property)
                .map(|channel| {
                    channel
                        .frames()
                        .into_iter()
                        .map(|local| KeyRef::new(property.clone(), clip.start() + local))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

pub(crate) fn header(cut: &Cut, outline: Outline, width: f32) -> Element {
    let keyframing = cut.keyframing();
    let graph = keyframing.is_graph();
    let speed = keyframing.plot() == Plot::Speed;
    let flipped = match speed {
        true => Plot::Value,
        false => Plot::Speed,
    };
    let listed = cut
        .selected_animated()
        .map(|clip| names(clip, &outline))
        .unwrap_or_default();
    let bar = row! {
        text(tr!("cut.keys.summary"))
            .caption()
            .no_wrap()
            .grow(1.0)
            .shrink(1.0);
        if graph {
            toggle(MaterialSymbol::SPEED, GLYPH, speed, move |cut: &mut Cut| {
                cut.set_plot(flipped)
            })
            .tooltip(tr!("cut.keys.speed_graph"));
            glyph_button(MaterialSymbol::FIT_SCREEN, GLYPH)
                .ghost()
                .on_press(Cut::fit_keys)
                .tooltip(tr!("cut.keys.fit"));
        }
        toggle(MaterialSymbol::SHOW_CHART, GLYPH, graph, Cut::toggle_graph)
            .tooltip(Command::GraphEditor.tooltip());
    }
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .height(SUMMARY)
    .fill_width();
    let names = Names {
        outline,
        names: listed,
        picked: cut.picked_keys(),
        plotted: std::sync::Arc::clone(keyframing.plotted()),
    };
    column! {
        bar;
        names.build().fill_width();
    }
    .background(palette().surface)
    .width(width)
    .build()
}
