use cut_project::{MarkerColor, MarkerScope, Timecode};
use rikka_app::prelude::*;
use rikka_widgets::{column, popover, row};

use crate::cut::Cut;
use crate::timeline::{markers, ruler};

const WIDTH: f32 = 280.0;
const SWATCH: f32 = 20.0;
const CHECK: f32 = 14.0;
const FIELD: f32 = 120.0;

pub(crate) fn editor(cut: &Cut) -> Option<Element> {
    let marking = cut.marking()?;
    let project = cut.project();
    let marker = project.marker(marking.scope, marking.id)?;
    let rate = project.rate();
    let (frame, title) = match marking.scope {
        MarkerScope::Timeline => (marker.at, tr!("cut.marker.timeline")),
        MarkerScope::Clip(clip) => {
            let (_, clip_ref) = project.locate(clip)?;
            let frame = clip_ref
                .marker_span(marker)
                .map_or(clip_ref.start(), |span| span.start);
            (frame, tr!("cut.marker.clip"))
        }
    };
    let per_second = rate.per_second() as f32;
    let seconds = marker.length as f32 / per_second;
    let x = cut.scale().x_of(frame as f64).max(0.0);
    let anchor = Anchor::new(ruler::KEY, AnchorEdge::BelowStart).offset(x, 0.0);
    let scope = marking.scope;
    let id = marking.id;
    let selected = marker.color;
    let swatches = row_of(MarkerColor::ALL.map(|color| swatch(color, color == selected)))
        .gap(Space::TIGHT)
        .align_items(Align::Center);
    let content = column! {
        row! {
            text(title).label().semibold().grow(1.0);
            text(Timecode::from_frame(frame, rate).to_string()).caption().muted();
        }
        .align_items(Align::Center)
        .fill_width();
        field(
            tr!("cut.marker.name"),
            text_field(&marking.name)
                .placeholder(tr!("cut.marker.unnamed"))
                .control_size(ControlSize::Small)
                .on_blur(Cut::commit_marker)
                .on_submit(Cut::commit_marker)
                .fill_width(),
        );
        field(tr!("cut.marker.color"), swatches);
        field(
            tr!("cut.marker.length"),
            number_field(seconds)
                .range(0.0..=f32::MAX)
                .unit(tr!("cut.unit.seconds"))
                .decimals(2)
                .control_size(ControlSize::Small)
                .on_commit(move |cut: &mut Cut, seconds: f32| {
                    let length = (seconds * per_second).round() as i64;
                    cut.change_marker(|marker| marker.length = length.max(0));
                })
                .width(FIELD),
        );
        field(
            tr!("cut.marker.note"),
            text_field(&marking.note)
                .placeholder(tr!("cut.marker.no_note"))
                .control_size(ControlSize::Small)
                .on_blur(Cut::commit_marker)
                .on_submit(Cut::commit_marker)
                .fill_width(),
        );
        row! {
            button(text(tr!("cut.marker.delete")))
                .ghost()
                .small()
                .on_press(move |cut: &mut Cut| cut.remove_marker(scope, id));
            spacer().grow(1.0);
            button(text(tr!("cut.marker.done")))
                .primary()
                .small()
                .on_press(Cut::close_marker);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .width(WIDTH);
    Some(
        popover(content)
            .on_dismiss(Cut::close_marker)
            .build()
            .keyed(("cut-marker", scope, id))
            .anchored(anchor),
    )
}

fn swatch(color: MarkerColor, selected: bool) -> Element {
    let ink = markers::color(color);
    let check = selected.then(|| {
        icon(MaterialSymbol::CHECK)
            .extent(Length::px(CHECK))
            .color(Color::BLACK)
    });
    container(row! {
        check;
    })
    .background(ink)
    .radius(Corner::Item)
    .center()
    .on_press(move |cut: &mut Cut| cut.change_marker(|marker| marker.color = color))
    .focusless()
    .size(SWATCH, SWATCH)
    .build()
    .keyed(("cut-marker-swatch", color))
}
