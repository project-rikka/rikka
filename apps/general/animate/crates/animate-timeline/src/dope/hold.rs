use animate_sheet::HoldSpan;
use animate_track::Edge;
use rikka_app::prelude::*;

use super::pour::{grip_on, nudge};
use crate::layout::{Geometry, HoldRows, Row};

const INSET: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HoldDrag {
    pub(crate) lane: usize,
    pub(crate) grip: Edge,
    anchor: f32,
    pub(crate) from: HoldSpan,
}

impl HoldDrag {
    pub(crate) fn to(&self, geometry: &Geometry, playhead: u32, x: f32) -> HoldSpan {
        HoldSpan {
            span: self
                .from
                .span
                .dragged(self.grip, self.anchor, geometry.frame_at(x), |frame| {
                    nudge(geometry, playhead, frame)
                }),
            ..self.from
        }
    }
}

pub(crate) fn bar(geometry: &Geometry, band: Rect, span: HoldSpan) -> Rect {
    let start = geometry.x_of(span.span.start as f32);
    let end = span
        .span
        .end
        .map_or(band.max_x(), |end| geometry.x_of(end as f32));
    Rect::new(
        start,
        band.y() + INSET,
        (end - start).max(1.0),
        band.height() - 2.0 * INSET,
    )
}

pub(crate) fn lead(geometry: &Geometry, bar: Rect, ease: u32) -> Rect {
    let width = ease as f32 * geometry.zoom();
    Rect::new(bar.min_x() - width, bar.y(), width, bar.height())
}

pub(crate) fn grab(geometry: &Geometry, rows: &HoldRows, point: Point) -> Option<HoldDrag> {
    let keys = geometry.keys();
    if !keys.contains(point) {
        return None;
    }
    let height = Row::Hold(0).height();
    let lane = ((point.y - geometry.top_at(rows.offset)) / height).floor();
    if lane < 0.0 {
        return None;
    }
    let lane = lane as usize;
    let row = rows.sheet.holding().get(lane)?;
    let band = Rect::new(
        keys.x(),
        geometry.top_at(rows.offset + lane as f32 * height),
        keys.width(),
        height,
    );
    row.spans.iter().find_map(|span| {
        Some(HoldDrag {
            lane,
            grip: grip_on(bar(geometry, band, *span), point.x, true)?,
            anchor: geometry.frame_at(point.x),
            from: *span,
        })
    })
}
