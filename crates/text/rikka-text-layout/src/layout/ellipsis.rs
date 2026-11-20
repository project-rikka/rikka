use std::ops::{ControlFlow, Range};

use crate::fonts::EllipsisGlyphs;
use crate::layout::breaking::WIDTH_EPSILON;
use crate::layout::clusters::for_each_cluster;
use crate::layout::ellipsis_glyphs::ellipsis_of;
use crate::layout::params::LayoutParams;
use crate::shaping::Shaped;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cut {
    pub(crate) end: u32,
    pub(crate) last_start: u32,
    pub(crate) face: Option<u16>,
    pub(crate) style: u16,
}

impl Cut {
    pub(crate) fn ellipsis<'a>(&self, shaped: &'a Shaped) -> Option<(u16, &'a EllipsisGlyphs)> {
        let primary = shaped.styles[usize::from(self.style)].primary;
        [primary, shaped.base_face(), self.face]
            .into_iter()
            .flatten()
            .find_map(|face| {
                ellipsis_of(&shaped.faces[usize::from(face)]).map(|ellipsis| (face, ellipsis))
            })
    }

    pub(crate) fn ellipsis_width(&self, shaped: &Shaped, font_size: f32) -> f32 {
        let size = shaped.styles[usize::from(self.style)].size(font_size);
        self.ellipsis(shaped)
            .map_or(0.0, |(_, ellipsis)| ellipsis.advance * size)
    }
}

pub(crate) const MAX_ATTEMPTS: usize = 4;

pub(crate) fn find_cut(
    shaped: &Shaped,
    range: Range<u32>,
    params: &LayoutParams,
    width: f32,
) -> Cut {
    let font_size = params.font_size;
    let limit = width + WIDTH_EPSILON;
    let opening = shaped.runs.get(shaped.run_at_offset(range.start));
    let mut best = Cut {
        end: range.start,
        last_start: range.start,
        face: opening.map(|run| run.face).or(shaped.base_face()),
        style: opening.map_or(0, |run| run.style),
    };
    for_each_cluster(shaped, range, params, 0.0, |cluster| {
        let right = cluster.x + cluster.width;
        if right > limit {
            return ControlFlow::Break(());
        }
        if !cluster.is_whitespace() {
            let run = &shaped.runs[cluster.run];
            let candidate = Cut {
                end: cluster.end,
                last_start: cluster.start,
                face: Some(run.face),
                style: run.style,
            };
            if right + candidate.ellipsis_width(shaped, font_size) <= limit {
                best = candidate;
            }
        }
        ControlFlow::Continue(())
    });
    best
}
