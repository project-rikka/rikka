use std::rc::Rc;
use std::time::Duration;

use paint_document::{Area, Region, Span};
use rikka_app::prelude::*;
use rikka_image_edit::{Erasing, Imaging, Marks, View};
use rikka_ui::{ComputedImage, ImageSource};

use crate::stroke::Stroke;

#[derive(Debug, Default)]
pub(crate) struct Overlay {
    computed: ComputedImage,
    frame: Option<OverlayFrame>,
    banded: Option<(Rc<Region>, Vec<Area>)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placing {
    pub(crate) view: View,
    pub(crate) offset: Point,
    pub(crate) erasing: Erasing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OverlayFrame {
    revision: u64,
    placing: Placing,
}

pub(crate) enum Shown {
    Unchanged(ImageSource),
    Redrawn(ImageSource),
}

impl Overlay {
    pub(crate) fn bands(&mut self, region: Rc<Region>) -> &[Area] {
        let stale = self
            .banded
            .as_ref()
            .is_none_or(|(cached, _)| !Rc::ptr_eq(cached, &region));
        if stale {
            let bands = bands(&region);
            self.banded = Some((region, bands));
        }
        self.banded.as_ref().map_or(&[], |(_, bands)| bands)
    }

    pub(crate) fn show(
        &mut self,
        stroke: &Stroke,
        revision: u64,
        placing: Placing,
    ) -> Option<Shown> {
        let key = OverlayFrame { revision, placing };
        let source = ImageSource::Computed(self.computed.clone());
        if self.frame == Some(key) {
            return Some(Shown::Unchanged(source));
        }
        let Some(compute) = rikka_picture::SharedGpu::compute(Duration::ZERO) else {
            tracing::warn!("stroke overlay requires GPU compute");
            return None;
        };
        let imaging = Imaging::shared(&compute);
        let marks = Marks::new(stroke.mark(placing.offset));
        match imaging.marks_overlay(&marks, placing.view, placing.erasing) {
            Ok(computed) => {
                self.computed.show(computed);
                self.frame = Some(key);
                Some(Shown::Redrawn(source))
            }
            Err(error) => {
                tracing::warn!(%error, "failed to draw stroke overlay");
                None
            }
        }
    }
}

fn bands(region: &Region) -> Vec<Area> {
    let footprint = region.footprint();
    let mut bands = Vec::new();
    let mut open: Option<(u32, u32, &[Span])> = None;
    for (y, spans) in footprint.rows() {
        match &mut open {
            Some((_, end, run_spans)) if *end == y && *run_spans == spans => *end = y + 1,
            _ => {
                if let Some(closed) = open.replace((y, y + 1, spans)) {
                    close(&mut bands, closed);
                }
            }
        }
    }
    if let Some(closed) = open {
        close(&mut bands, closed);
    }
    bands
}

fn close(bands: &mut Vec<Area>, (top, bottom, spans): (u32, u32, &[Span])) {
    bands.extend(
        spans
            .iter()
            .map(|span| Area::new(span.start, top, span.end - span.start, bottom - top)),
    );
}
