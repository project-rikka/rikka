use rikka_types::{Color, Point, Rect};
use rikka_ui::{Element, Layout, PaintCtx, Quad, Style, Styled, View, Widget};

use crate::widgets::paint::stroke::stroke;

const LINE: f32 = 1.5;
const AREA_ALPHA: f32 = 0.16;
const LEAST: f32 = 0.02;
const BASELINE: f32 = 1.0;
const BASELINE_ALPHA: f32 = 0.18;
const NOW: f32 = 5.0;
const GRID_LINES: usize = 4;
const GRID_ALPHA: f32 = 0.06;

#[derive(Debug)]
pub(crate) struct Sparkline {
    values: Vec<f32>,
    room: usize,
    color: Color,
    gridded: bool,
    style: Style,
}

#[must_use]
pub(crate) fn sparkline(
    values: impl IntoIterator<Item = f32>,
    room: usize,
    color: Color,
) -> Sparkline {
    Sparkline {
        values: values
            .into_iter()
            .map(|value| value.clamp(0.0, 1.0))
            .collect(),
        room: room.max(2),
        color,
        gridded: false,
        style: Style::default(),
    }
}

impl Sparkline {
    #[must_use]
    pub(crate) fn gridded(mut self) -> Sparkline {
        self.gridded = true;
        self
    }
}

impl Widget for Sparkline {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.width() <= 0.0 || bounds.height() <= LINE {
            return;
        }
        ctx.draw_quad(
            Quad::new(Rect::new(
                bounds.x(),
                bounds.max_y() - BASELINE,
                bounds.width(),
                BASELINE,
            ))
            .fill(self.color.scale_alpha(BASELINE_ALPHA)),
        );
        if self.gridded {
            let grid = ctx.env().text_color.scale_alpha(GRID_ALPHA);
            for step in 1..GRID_LINES {
                let y = bounds.max_y() - bounds.height() * step as f32 / GRID_LINES as f32;
                ctx.draw_quad(
                    Quad::new(Rect::new(bounds.x(), y.round(), bounds.width(), BASELINE))
                        .fill(grid),
                );
            }
        }
        if self.values.is_empty() {
            return;
        }
        let pitch = (bounds.width() - NOW / 2.0) / (self.room - 1) as f32;
        let first = self.room.saturating_sub(self.values.len());
        let reach = bounds.height() - LINE;
        let points: Vec<Point> = self
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                Point::new(
                    bounds.x() + pitch * (first + index) as f32,
                    bounds.max_y() - LINE / 2.0 - reach * value.max(LEAST),
                )
            })
            .collect();
        let area = self.color.scale_alpha(AREA_ALPHA);
        for point in &points {
            let left = (point.x - pitch / 2.0).max(bounds.x());
            let right = (point.x + pitch / 2.0).min(bounds.max_x());
            ctx.draw_quad(
                Quad::new(Rect::new(
                    left,
                    point.y,
                    right - left,
                    bounds.max_y() - point.y,
                ))
                .fill(area),
            );
        }
        for pair in points.windows(2) {
            stroke(ctx, pair[0], pair[1], LINE, self.color);
        }
        if let Some(last) = points.last() {
            ctx.draw_circle(*last, NOW, self.color);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.values == other.values
            && self.room == other.room
            && self.color == other.color
            && self.gridded == other.gridded
            && self.style == other.style
    }
}

impl Styled for Sparkline {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl View for Sparkline {
    fn build(self) -> Element {
        Element::new(self)
    }
}
