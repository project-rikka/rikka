use paint_document::{Backdrop, Blank};
use rikka_app::prelude::*;
use rikka_effects_forms::labelled;
use rikka_widgets::column;

use crate::paint::Paint;
use crate::session::form;
use crate::view::STAGE;

const WIDTH: f32 = 420.0;
const FIELD: f32 = 120.0;
const MAX_RESOLUTION: f32 = 10_000.0;
const FILLS: [Fill; 3] = [Fill::White, Fill::Primary, Fill::Transparent];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fill {
    White,
    Primary,
    Transparent,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NewImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) resolution: f32,
    pub(crate) fill: Fill,
}

impl Default for NewImage {
    fn default() -> NewImage {
        NewImage {
            width: 1920,
            height: 1080,
            resolution: 72.0,
            fill: Fill::White,
        }
    }
}

impl NewImage {
    pub(crate) fn blank(&self, primary: Color, layer_name: String) -> Blank {
        let backdrop = match self.fill {
            Fill::White => Backdrop::Color(Color::WHITE),
            Fill::Primary => Backdrop::Color(primary.with_alpha(1.0)),
            Fill::Transparent => Backdrop::Transparent,
        };
        Blank::new(self.width, self.height)
            .resolution(f64::from(self.resolution))
            .backdrop(backdrop)
            .layer_name(layer_name)
    }
}

pub(crate) fn view(paint: &Paint) -> Vec<Element> {
    let Some(creating) = paint.creating() else {
        return Vec::new();
    };
    let at = FILLS
        .iter()
        .position(|fill| *fill == creating.fill)
        .unwrap_or(0);
    let content = column! {
        labelled(
            tr!("paint.new_image.width"),
            size(creating.width, |paint: &mut Paint, width: f32| {
                paint.tune_new_image(|new| new.width = width.round().max(1.0) as u32);
            }),
        );
        labelled(
            tr!("paint.new_image.height"),
            size(creating.height, |paint: &mut Paint, height: f32| {
                paint.tune_new_image(|new| new.height = height.round().max(1.0) as u32);
            }),
        );
        labelled(
            tr!("paint.new_image.resolution"),
            number_field(creating.resolution)
                .range(1.0..=MAX_RESOLUTION)
                .decimals(0)
                .unit(tr!("paint.new_image.per_inch"))
                .on_commit(|paint: &mut Paint, resolution: f32| {
                    paint.tune_new_image(|new| new.resolution = resolution);
                })
                .on_submit(Paint::create_image)
                .width(FIELD),
        );
        labelled(
            tr!("paint.new_image.background"),
            segmented(at)
                .segments([
                    tr!("paint.new_image.white"),
                    tr!("paint.new_image.primary"),
                    tr!("paint.new_image.transparent"),
                ])
                .on_select(|paint: &mut Paint, at: usize| {
                    paint.tune_new_image(|new| new.fill = FILLS[at]);
                })
                .fill_width(),
        );
    }
    .gap(Space::ROOMY)
    .fill_width();
    dialog(tr!("paint.new_image.title"), content)
        .cancel(tr!("paint.new_image.cancel"), Paint::cancel_new_image)
        .accept(tr!("paint.new_image.create"), Paint::create_image)
        .width(WIDTH)
        .keyed("paint-new-image")
        .centered_on(STAGE)
        .with_scrim()
}

fn size(value: u32, on_change: impl Fn(&mut Paint, f32) + Clone + 'static) -> impl View {
    number_field(value as f32)
        .range(1.0..=form::MAX_PIXELS)
        .unit(tr!("paint.unit.pixels"))
        .on_commit(on_change)
        .on_submit(Paint::create_image)
        .width(FIELD)
}
