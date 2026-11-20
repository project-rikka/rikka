use std::time::{Duration, Instant};

use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_theme::{Corner, Theme};
use rikka_types::{Point, Rect, RoundedRect, Size};
use rikka_ui::motion::indicator::{self, Indicator};
use rikka_ui::scroll::bar::{self, Scroll};
use rikka_ui::text::TextRun;
use rikka_ui::{Axis, PaintCtx, Quad, measure_text, motion};

use crate::control::ControlSize;
use crate::navigation::menu::slider::{MenuSlider, TRACK_WIDTH};
use crate::navigation::menu::{Kind, MenuItem};
use crate::rows;

pub(crate) const PADDING: f32 = 6.0;

const ROW_HEIGHT: f32 = 0.72;

pub(crate) const SEPARATOR_HEIGHT: f32 = 11.0;
const SEPARATOR_INSET: f32 = 8.0;

pub(crate) const DEFAULT_WIDTH: f32 = 200.0;

const HEADER_FONT: f32 = 0.85;

const OPEN: &str = "menu.open";

const AIM_PATIENCE: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
pub(crate) struct Panel<'a> {
    pub(crate) items: &'a [MenuItem],
    pub(crate) size: ControlSize,
    pub(crate) header: Option<f32>,
}

impl<'a> Panel<'a> {
    pub(crate) fn new(items: &'a [MenuItem], size: ControlSize) -> Self {
        Self {
            items,
            size,
            header: None,
        }
    }

    pub(crate) fn row_height(&self) -> f32 {
        self.size.height() * ROW_HEIGHT
    }

    pub(crate) fn total_height(&self) -> f32 {
        self.header.unwrap_or_default() + self.rows_height()
    }

    fn rows_height(&self) -> f32 {
        let row = self.row_height();
        self.items.iter().map(|item| item.height(row)).sum::<f32>() + PADDING * 2.0
    }

    pub(crate) fn rows(&self, bounds: Rect) -> Rect {
        let header = self.header.unwrap_or_default();
        Rect::new(
            bounds.x(),
            bounds.y() + header,
            bounds.width(),
            (bounds.height() - header).max(0.0),
        )
    }

    fn furthest(&self, bounds: Rect) -> f32 {
        bar::max_scroll(Axis::Vertical, self.rows(bounds), self.rows_height())
    }

    fn lane(&self, furthest: f32) -> f32 {
        match furthest > 0.0 {
            true => bar::lane(PADDING),
            false => 0.0,
        }
    }

    fn gutters(&self) -> (f32, f32) {
        let width = self.size.icon_size() + self.size.gap();
        let reserve = |needed: bool| if needed { width } else { 0.0 };
        (
            reserve(self.header.is_some() || self.items.iter().any(|item| item.selected)),
            reserve(self.items.iter().any(|item| item.icon.is_some())),
        )
    }

    pub(crate) fn natural_width(&self, theme: &Theme) -> f32 {
        let font_size = self.size.font_size(theme);
        let inset = self.size.gap();
        let (check, icons) = self.gutters();
        let mut widest: f32 = 0.0;
        let mut trailing: f32 = 0.0;
        for item in self
            .items
            .iter()
            .filter(|item| item.kind != Kind::Separator)
        {
            let label = TextRun::new(item.label.clone(), font_size).no_wrap();
            let mut needed = measure_text(&label, None).width;
            if item.opens() {
                needed += font_size + inset;
            } else if item.slider.is_some() {
                needed += TRACK_WIDTH + inset;
            } else if let Some(shortcut) = &item.shortcut {
                let run = TextRun::new(shortcut.clone(), font_size).no_wrap();
                let width = measure_text(&run, None).width;
                trailing = trailing.max(width);
                needed += width + inset;
            }
            widest = widest.max(needed);
        }
        let rows = (inset * 2.0 + check + icons + widest).max(rows::room_for_trailing(trailing));
        (rows + PADDING * 2.0).ceil()
    }

    pub(crate) fn size_beside(&self, theme: &Theme) -> Size {
        Size::new(
            self.natural_width(theme).max(DEFAULT_WIDTH),
            self.total_height(),
        )
    }

    pub(crate) fn opens(&self, row: Option<usize>) -> bool {
        row.is_some_and(|row| self.items.get(row).is_some_and(MenuItem::opens))
    }

    pub(crate) fn branch(&self, row: usize) -> Option<&'a [MenuItem]> {
        let item = self.items.get(row)?;
        (item.is_command() && !item.children.is_empty()).then_some(item.children.as_slice())
    }

    pub(crate) fn row_rect(&self, bounds: Rect, offset: f32, index: usize) -> Rect {
        let rows = self.rows(bounds);
        let row = self.row_height();
        Rect::new(
            PADDING,
            rows.y() - bounds.y() + PADDING - offset + self.row_top(index),
            (rows.width() - PADDING * 2.0 - self.lane(self.furthest(bounds))).max(0.0),
            self.items.get(index).map_or(row, |item| item.height(row)),
        )
    }

    pub(crate) fn is_slider(&self, row: usize) -> bool {
        self.items
            .get(row)
            .is_some_and(|item| item.slider.is_some())
    }

    pub(crate) fn slider_value(
        &self,
        bounds: Rect,
        offset: f32,
        row: usize,
        x: f32,
    ) -> Option<f32> {
        let slider = self.items.get(row)?.slider.as_ref()?;
        let track = self.track(bounds, offset, row);
        Some(slider.value_at(track, x))
    }

    pub(crate) fn slider_hit(&self, bounds: Rect, offset: f32, row: usize, x: f32) -> Option<f32> {
        let track = self.track(bounds, offset, row);
        let reach = track.x() - self.size.gap()..=track.max_x() + self.size.gap();
        reach
            .contains(&x)
            .then(|| self.slider_value(bounds, offset, row, x))
            .flatten()
    }

    fn track(&self, bounds: Rect, offset: f32, row: usize) -> Rect {
        let rect = self.row_rect(bounds, offset, row).translate(bounds.origin);
        MenuSlider::track(rect, self.size.gap())
    }

    pub(crate) fn command_at(&self, bounds: Rect, offset: f32, y: f32) -> Option<usize> {
        let rows = self.rows(bounds);
        if y < rows.y() || y >= rows.max_y() {
            return None;
        }
        let row = self.row_height();
        let mut top = rows.y() + PADDING - offset;
        for (index, item) in self.items.iter().enumerate() {
            let bottom = top + item.height(row);
            if y < bottom {
                return (y >= top && item.is_command()).then_some(index);
            }
            top = bottom;
        }
        None
    }

    pub(crate) fn scroll_by(&self, state: &mut Listing, bounds: Rect, travel: f32) -> bool {
        state.scroll.nudge(travel, self.furthest(bounds))
    }

    pub(crate) fn reveal(&self, state: &mut Listing, bounds: Rect, index: usize) {
        if let Some(offset) = self.revealing(state, bounds, index) {
            state.scroll.aim(offset);
        }
    }

    pub(crate) fn reveal_at_once(&self, state: &mut Listing, bounds: Rect, index: usize) {
        if let Some(offset) = self.revealing(state, bounds, index) {
            state.scroll.snap(offset);
        }
    }

    fn revealing(&self, state: &Listing, bounds: Rect, index: usize) -> Option<f32> {
        let item = self.items.get(index)?;
        let top = PADDING + self.row_top(index);
        let extent = item.height(self.row_height()) + PADDING;
        let room = self.rows(bounds).height();
        let brought = bar::reveal(state.scroll.target(), top, extent, room);
        Some(brought.clamp(0.0, self.furthest(bounds)))
    }

    pub(crate) fn page_rows(&self, bounds: Rect) -> usize {
        let room = self.rows(bounds).height() - PADDING * 2.0;
        (room / self.row_height()).floor().max(1.0) as usize
    }

    fn row_top(&self, index: usize) -> f32 {
        let row = self.row_height();
        self.items[..index.min(self.items.len())]
            .iter()
            .map(|item| item.height(row))
            .sum()
    }

    fn paint_row(
        &self,
        ctx: &mut PaintCtx<'_>,
        item: &MenuItem,
        row: Rect,
        emphasis: f32,
        (check, icons): (f32, f32),
    ) {
        let palette = ctx.theme().palette();
        let font_size = self.size.font_size(ctx.theme());
        let inset = self.size.gap();
        let glyph = self.size.icon_size();

        match item.kind {
            Kind::Separator => {
                let line = Rect::new(
                    row.x() + SEPARATOR_INSET,
                    row.center().y,
                    (row.width() - SEPARATOR_INSET * 2.0).max(0.0),
                    1.0,
                );
                ctx.draw_quad(Quad::new(line).fill(palette.outline));
                return;
            }
            Kind::Header => {
                let run = TextRun::new(item.label.clone(), font_size * HEADER_FONT)
                    .medium()
                    .no_wrap();
                let x = row.x() + inset + check + icons;
                let width = (row.max_x() - inset - x).max(0.0);
                rows::draw_cell(ctx, row, x, width, run, palette.text_muted);
                return;
            }
            Kind::Item => {}
        }

        let (label_color, hint_color) = if !item.enabled {
            (
                ctx.theme().disabled(palette.text),
                ctx.theme().disabled(palette.text_muted),
            )
        } else {
            (
                palette.text.mix(palette.on_selection, emphasis),
                palette.text_muted.mix(palette.on_selection, emphasis),
            )
        };

        let mut label_x = row.x() + inset;
        if item.selected {
            ctx.draw_icon(
                rows::glyph_rect(label_x, row.center().y, glyph),
                IconSpec::new(MaterialSymbol::CHECK.codepoint(), glyph),
                label_color,
            );
        }
        label_x += check;
        if let Some(icon) = &item.icon {
            ctx.draw_icon_source(
                rows::glyph_rect(label_x, row.center().y, glyph),
                icon,
                glyph,
                label_color,
            );
        }
        label_x += icons;

        let mut width = (row.max_x() - inset - label_x).max(0.0);
        if let Some(slider) = &item.slider {
            let track = MenuSlider::track(row, inset);
            slider.paint(ctx, track, item.enabled);
            width = (track.x() - inset - label_x).max(0.0);
        } else if item.opens() {
            width = (width - font_size - inset).max(0.0);
            ctx.draw_icon(
                Rect::from_center(
                    Point::new(row.max_x() - inset - font_size / 2.0, row.center().y),
                    Size::new(font_size, font_size),
                ),
                IconSpec::new(MaterialSymbol::CHEVRON_RIGHT.codepoint(), font_size),
                hint_color,
            );
        } else if let Some(shortcut) = &item.shortcut {
            let run = TextRun::new(shortcut.clone(), font_size).no_wrap();
            width = rows::draw_trailing(ctx, row, inset, width, run, hint_color);
        }

        let run = TextRun::new(item.label.clone(), font_size).no_wrap();
        rows::draw_cell(ctx, row, label_x, width, run, label_color);
    }

    pub(crate) fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
        let count = self.items.len();
        if count == 0 {
            return None;
        }
        let start = match from {
            Some(index) => index,
            None if forward => count - 1,
            None => 0,
        };
        for hop in 1..=count {
            let index = if forward {
                (start + hop) % count
            } else {
                (start + count - hop % count) % count
            };
            if self.items[index].is_command() {
                return Some(index);
            }
        }
        None
    }

    pub(crate) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        state: &mut Listing,
        bounds: Rect,
        shown: f32,
        pivot: Point,
        header: impl FnOnce(&mut PaintCtx<'_>, Rect),
    ) {
        if shown <= 0.0 {
            return;
        }
        let palette = ctx.theme().palette();
        let radius = Corner::Panel.radius();
        let row_radius = Corner::Panel.nested(PADDING);
        let rows = self.rows(bounds);
        let content = self.rows_height();
        let furthest = bar::max_scroll(Axis::Vertical, rows, content);
        let scrolled = state.scroll.advance(ctx, Axis::Vertical, furthest);

        let lane = self.lane(furthest);
        let hovered = ctx.hovered();
        ctx.popped(shown, pivot, |ctx| {
            ctx.draw_pane(
                Quad::new(bounds)
                    .fill(palette.surface)
                    .stroke(1.0, palette.outline)
                    .radius(radius),
            );
            ctx.clipped(RoundedRect::new(bounds, radius), |ctx| {
                match self.header {
                    Some(band) => {
                        ctx.clipped(rows, |ctx| {
                            self.paint_rows(ctx, state, rows, row_radius, lane);
                        });
                        header(ctx, Rect::new(bounds.x(), bounds.y(), bounds.width(), band));
                    }
                    None => self.paint_rows(ctx, state, bounds, row_radius, lane),
                }
                bar::paint_thumb(
                    ctx,
                    Axis::Vertical,
                    rows,
                    content,
                    scrolled,
                    (hovered, false),
                );
            });
        });
    }

    fn paint_rows(
        &self,
        ctx: &mut PaintCtx<'_>,
        state: &mut Listing,
        bounds: Rect,
        row_radius: f32,
        lane: f32,
    ) {
        let palette = ctx.theme().palette();
        let row_height = self.row_height();
        let width = (bounds.width() - PADDING * 2.0 - lane).max(0.0);
        let origin = bounds.y() + PADDING - state.scroll.rendered();

        let place = state
            .highlighted
            .filter(|index| self.items.get(*index).is_some_and(MenuItem::is_command))
            .map(|index| (index, origin - bounds.y() + self.row_top(index)));
        let highlight = state
            .highlight
            .resolve(
                ctx,
                "highlight",
                place,
                palette.selection,
                motion::INDICATOR,
            )
            .map(|(y, fill)| (bounds.y() + y, fill));
        if let Some((y, fill)) = highlight {
            ctx.draw_quad(
                Quad::new(Rect::new(bounds.x() + PADDING, y, width, row_height))
                    .fill(fill)
                    .radius(row_radius),
            );
        }

        let gutters = self.gutters();
        let mut top = origin;
        for item in self.items {
            let height = item.height(row_height);
            let row = Rect::new(bounds.x() + PADDING, top, width, height);
            top += height;
            if row.max_y() < bounds.y() || row.y() > bounds.max_y() {
                continue;
            }
            let emphasis = match highlight {
                Some((y, _)) if item.is_command() => indicator::emphasis(y, row.y(), row_height),
                _ => 0.0,
            };
            self.paint_row(ctx, item, row, emphasis, gutters);
        }
    }
}

pub(crate) fn aiming(toward: Rect, from: Point, to: Point) -> bool {
    let rightward = toward.center().x >= from.x;
    let edge = match rightward {
        true => toward.x(),
        false => toward.max_x(),
    };
    let span = (edge - from.x).abs();
    let travelled = match rightward {
        true => to.x - from.x,
        false => from.x - to.x,
    };
    if span <= 0.0 || travelled < 0.0 {
        return false;
    }
    let reach = (travelled / span).min(1.0);
    let top = from.y + (toward.y() - from.y) * reach;
    let bottom = from.y + (toward.max_y() - from.y) * reach;
    (top..=bottom).contains(&to.y)
}

#[derive(Debug, Default)]
pub(crate) struct Listing {
    highlighted: Option<usize>,
    aim: Option<(Point, Instant)>,
    highlight: Indicator,
    scroll: Scroll,
    drawn: f32,
    unseen: bool,
}

impl Listing {
    pub(crate) fn unseen() -> Self {
        let mut scroll = Scroll::default();
        scroll.snap(0.0);
        Self {
            scroll,
            unseen: true,
            ..Self::default()
        }
    }

    pub(crate) fn reveal(&mut self, ctx: &mut PaintCtx<'_>, open: bool) -> f32 {
        if std::mem::take(&mut self.unseen) {
            ctx.restart(OPEN);
        }
        self.drawn = ctx.progress(OPEN, open, motion::POP, motion::POP_OUT);
        self.drawn
    }

    pub(crate) fn shown(&self) -> f32 {
        self.drawn
    }

    pub(crate) fn scrolled(&self) -> f32 {
        self.scroll.rendered()
    }

    pub(crate) fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    pub(crate) fn highlight(&mut self, next: Option<usize>) -> bool {
        if self.highlighted == next {
            return false;
        }
        self.highlighted = next;
        self.aim = None;
        true
    }

    pub(crate) fn relist(&mut self) {
        self.highlight(None);
        self.scroll.aim(0.0);
    }

    pub(crate) fn aim_from(&mut self, at: Point, now: Instant) {
        self.aim = Some((at, now));
    }

    pub(crate) fn drop_aim(&mut self) {
        self.aim = None;
    }

    pub(crate) fn holds_aim(&self, toward: Rect, to: Point, now: Instant) -> bool {
        self.aim.is_some_and(|(from, since)| {
            now.saturating_duration_since(since) < AIM_PATIENCE && aiming(toward, from, to)
        })
    }
}
