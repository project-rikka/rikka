use cut_project::Format;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Reformatting;
use super::draft::SIZES;
use crate::controls::glyph_button;
use crate::cut::Cut;
use crate::view::STAGE;

const WIDTH: f32 = 420.0;
const FIELD: f32 = 110.0;
const GLYPH: f32 = 18.0;

pub(crate) fn sheet(reformatting: &Reformatting) -> Vec<Element> {
    let format = reformatting.format;
    let mut sizes: Vec<Label> = SIZES
        .iter()
        .map(|(width, height)| size_name(*width, *height))
        .collect();
    let selected_size = reformatting.size().unwrap_or_else(|| {
        sizes.push(tr!("cut.reformat.custom"));
        SIZES.len()
    });
    let rates = reformatting.rates();
    let selected_rate = rates.iter().position(|rate| *rate == format.rate);
    let rate_names: Vec<Label> = rates
        .iter()
        .map(|rate| tr!("cut.reformat.rate", rate = &rate.to_string()))
        .collect();
    let (across, down) = format.aspect();
    let side = |value: u32| {
        number_field(value as f32)
            .range(*Format::SIDES.start() as f32..=*Format::SIDES.end() as f32)
            .unit(tr!("cut.unit.pixels"))
            .decimals(0)
            .control_size(ControlSize::Small)
            .width(FIELD)
    };
    let body = column! {
        field(
            tr!("cut.reformat.size"),
            combo(selected_size)
                .options(sizes)
                .control_size(ControlSize::Small)
                .on_select(|cut: &mut Cut, at: usize| {
                    if let Some((width, height)) = SIZES.get(at) {
                        cut.tune_reformat(|draft| draft.sized(*width, *height));
                    }
                })
                .fill_width(),
        );
        row! {
            side(format.width).on_commit(|cut: &mut Cut, width: f32| {
                cut.tune_reformat(|draft| draft.sized(width as u32, draft.format.height));
            });
            text("×").color(TextColor::Muted);
            side(format.height).on_commit(|cut: &mut Cut, height: f32| {
                cut.tune_reformat(|draft| draft.sized(draft.format.width, height as u32));
            });
            glyph_button(MaterialSymbol::SCREEN_ROTATION, GLYPH)
                .ghost()
                .on_press(|cut: &mut Cut| cut.tune_reformat(Reformatting::turned))
                .tooltip(tr!("cut.reformat.swap"));
            spacer().grow(1.0);
            text(tr!(
                "cut.reformat.aspect",
                across = &across.to_string(),
                down = &down.to_string()
            ))
            .caption()
            .muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        field(
            tr!("cut.reformat.rate_label"),
            combo(selected_rate)
                .options(rate_names)
                .control_size(ControlSize::Small)
                .on_select(move |cut: &mut Cut, at: usize| {
                    if let Some(rate) = rates.get(at).copied() {
                        cut.tune_reformat(|draft| draft.format.rate = rate);
                    }
                })
                .fill_width(),
        );
    }
    .gap(Space::ROOMY)
    .fill_width();
    dialog(tr!("cut.reformat.title"), body)
        .cancel(tr!("cut.reformat.cancel"), Cut::close_reformat)
        .accept(tr!("cut.reformat.apply"), Cut::reformat)
        .width(WIDTH)
        .keyed("cut-reformat")
        .centered_on(STAGE)
        .with_scrim()
}

fn size_name(width: u32, height: u32) -> Label {
    tr!(
        "cut.reformat.dimensions",
        width = &width.to_string(),
        height = &height.to_string()
    )
}
