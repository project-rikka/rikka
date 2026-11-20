use std::ops::ControlFlow;

use crate::layout::clusters::{Cluster, for_each_cluster};
use crate::layout::params::{LayoutParams, Wrap};
use crate::layout::plan::RowPlan;
use crate::shaping::{Segment, Shaped};

pub(crate) const WIDTH_EPSILON: f32 = 1e-3;

#[derive(Clone, Copy, Debug, Default)]
struct Widths {
    content: f32,
    trailing: f32,
    content_separators: u32,
    trailing_separators: u32,
}

#[derive(Clone, Copy, Debug)]
struct RowAcc {
    start: u32,
    committed: f32,
    content: f32,
    content_end: u32,
    separators: u32,
    pending: u32,
    occupied: bool,
}

impl RowAcc {
    fn new(start: u32) -> Self {
        Self {
            start,
            committed: 0.0,
            content: 0.0,
            content_end: start,
            separators: 0,
            pending: 0,
            occupied: false,
        }
    }

    fn add(&mut self, widths: Widths, content_end: Option<u32>) {
        self.occupied = true;
        if let Some(content_end) = content_end {
            self.content = self.committed + widths.content;
            self.content_end = content_end;
            self.separators += self.pending + widths.content_separators;
            self.pending = widths.trailing_separators;
        } else {
            self.pending += widths.trailing_separators;
        }
        self.committed += widths.content + widths.trailing;
    }

    fn add_cluster(&mut self, cluster: &Cluster) {
        let separator = u32::from(cluster.is_word_separator());
        let widths = if cluster.is_whitespace() {
            Widths {
                trailing: cluster.width,
                trailing_separators: separator,
                ..Widths::default()
            }
        } else {
            Widths {
                content: cluster.width,
                content_separators: separator,
                ..Widths::default()
            }
        };
        let content_end = (!cluster.is_whitespace()).then_some(cluster.end);
        self.add(widths, content_end);
    }
}

pub(crate) struct Breaker<'a> {
    shaped: &'a Shaped,
    params: &'a LayoutParams,
    limit: f32,
    wrap: Wrap,
    plans: &'a mut Vec<RowPlan>,
    row: RowAcc,
}

impl<'a> Breaker<'a> {
    pub(crate) fn new(
        shaped: &'a Shaped,
        params: &'a LayoutParams,
        plans: &'a mut Vec<RowPlan>,
    ) -> Self {
        plans.clear();
        Self {
            shaped,
            params,
            limit: params.limit() + WIDTH_EPSILON,
            wrap: params.wrap,
            plans,
            row: RowAcc::new(0),
        }
    }

    pub(crate) fn run(mut self) {
        let shaped = self.shaped;
        for segment in shaped.segments.iter() {
            match self.wrap {
                Wrap::Glyph => self.place_clusters(segment),
                _ => self.place_segment(segment),
            }
            if segment.mandatory {
                self.push_row(segment.end, true);
            }
        }
        let ended_by_break = self.plans.last().is_some_and(|plan| plan.mandatory);
        if self.row.occupied || self.plans.is_empty() || ended_by_break {
            self.push_row(shaped.text.len() as u32, false);
        }
    }

    fn place_segment(&mut self, segment: &Segment) {
        let mut widths = self.widths(segment, self.row.committed);
        let fits =
            |row: &RowAcc, widths: &Widths, limit: f32| row.committed + widths.content <= limit;
        if self.row.occupied && segment.has_content() && !fits(&self.row, &widths, self.limit) {
            self.push_row(segment.start, false);
            if segment.tab {
                widths = self.widths(segment, 0.0);
            }
        }
        let overlong = segment.has_content() && widths.content > self.limit;
        if overlong && self.wrap == Wrap::WordOrGlyph {
            self.place_clusters(segment);
            return;
        }
        let content_end = segment.has_content().then_some(segment.content_end);
        self.row.add(widths, content_end);
    }

    fn place_clusters(&mut self, segment: &Segment) {
        let shaped = self.shaped;
        let mut x = self.row.committed;
        let mut restart = true;
        let mut from = segment.start;
        while restart {
            restart = false;
            for_each_cluster(shaped, from..segment.end, self.params, x, |cluster| {
                let overflows = self.row.committed + cluster.width > self.limit;
                if !cluster.is_whitespace() && self.row.occupied && overflows {
                    self.push_row(cluster.start, false);
                    if shaped.tabs {
                        from = cluster.start;
                        x = 0.0;
                        restart = true;
                        return ControlFlow::Break(());
                    }
                }
                self.row.add_cluster(cluster);
                ControlFlow::Continue(())
            });
        }
    }

    fn widths(&self, segment: &Segment, x: f32) -> Widths {
        segment_widths(self.shaped, self.params, segment, x)
    }

    fn push_row(&mut self, end: u32, mandatory: bool) {
        let row = self.row;
        self.plans.push(RowPlan {
            start: row.start,
            end,
            content_end: row.content_end,
            separators: row.separators,
            mandatory,
            ..RowPlan::default()
        });
        self.row = RowAcc::new(end);
    }
}

pub(crate) fn widest_segment(shaped: &Shaped, params: &LayoutParams) -> f32 {
    shaped
        .segments
        .iter()
        .map(|segment| segment_widths(shaped, params, segment, 0.0).content)
        .fold(0.0, f32::max)
}

pub(crate) fn widest_cluster(shaped: &Shaped, params: &LayoutParams) -> f32 {
    let mut widest: f32 = 0.0;
    for_each_cluster(
        shaped,
        0..shaped.text.len() as u32,
        params,
        0.0,
        |cluster| {
            if !cluster.is_whitespace() {
                widest = widest.max(cluster.width);
            }
            ControlFlow::Continue(())
        },
    );
    widest
}

fn segment_widths(shaped: &Shaped, params: &LayoutParams, segment: &Segment, x: f32) -> Widths {
    let LayoutParams {
        font_size,
        letter_spacing,
        ..
    } = *params;
    if !segment.tab {
        return Widths {
            content: segment.content.to_px(font_size, letter_spacing),
            trailing: segment.trailing.to_px(font_size, letter_spacing),
            content_separators: segment.content_separators,
            trailing_separators: segment.trailing_separators,
        };
    }
    let mut row = RowAcc::new(segment.start);
    for_each_cluster(shaped, segment.start..segment.end, params, x, |cluster| {
        row.add_cluster(cluster);
        ControlFlow::Continue(())
    });
    Widths {
        content: row.content,
        trailing: row.committed - row.content,
        content_separators: row.separators,
        trailing_separators: row.pending,
    }
}
