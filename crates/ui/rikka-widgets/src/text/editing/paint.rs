use std::ops::Range;

use rikka_text::{Affinity, TextSpan, Wrap};
use rikka_theme::Syntax;
use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::scroll::bar::{self, Lanes};
use rikka_ui::search::Marking;
use rikka_ui::text::{DisplayText, TextRun};
use rikka_ui::{Axis, PaintCtx, Quad, motion};

use crate::rows::draw_cell;
use crate::text::editing::block::{Lines, Look, Piece};
use crate::text::editing::state::{CaretPosition, EditingState, Frame};
use crate::text::editing::{Editing, Room, walk};
use crate::text::{caret, preedit};

const SLACK: f32 = 1.0;

const SETTLE: u8 = 3;

const COUNTING: std::time::Duration = std::time::Duration::from_millis(50);

const PARSE_BUDGET: std::time::Duration = std::time::Duration::from_millis(3);

const LAID_PAGES: u32 = 1;

const KEPT_PAGES: u32 = 2;

struct Sheet<'a> {
    shown: &'a DisplayText<'a>,
    marking: Option<Marking>,
    selection: Option<(Range<usize>, Color)>,
    animated: Option<(Vec<Rect>, Color)>,
    composition: Option<[(Range<usize>, f32, Color); 2]>,
    ink: Color,
}

impl Editing {
    pub(crate) fn paint(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        room: Room,
        template: &TextRun,
    ) {
        let palette = ctx.theme().palette();
        self.find(state, ctx);

        if state.template.as_ref() != Some(template) {
            state.template = Some(template.clone());
        }
        state.frame = self.fit(state, &room, template);
        let wrap_width = self.wraps().then_some(state.frame.area.width());

        state.down.adopt(self.place.as_ref(), None);
        state.follow |= state.pasted.take();
        if state.follow || (self.options.single_line && self.stirred(state)) {
            self.follow_caret(state, template, wrap_width);
        }
        let frame = state.frame;
        let deepest = bar::max_scroll(Axis::Vertical, frame.room, frame.content_height);
        let page = frame.room.height();
        state.down.heed(self.place.as_ref(), deepest, page);
        let down = state.down.advance(ctx, Axis::Vertical, deepest);
        let stand = state.down.stand(deepest, page);
        state.down.publish(self.place.as_ref(), stand);
        let across = state.across.advance(
            ctx,
            Axis::Horizontal,
            bar::max_scroll(Axis::Horizontal, frame.room, frame.content_width),
        );
        let content = self.document.content();
        let shown = self.shown(&content);
        let mut visible = self.visible_lines(state, down, &frame, template.line_height);
        if state.rows.settle(&shown, template, visible.clone()) {
            visible = self.visible_lines(state, down, &frame, template.line_height);
            state.frame.content_height =
                state.rows.total() as f32 * template.line_height + room.top * 2.0;
        }
        if state.rows.counting() {
            let soon = ctx.now() + COUNTING;
            ctx.wake_at(soon);
        }
        state.frame.origin = frame.area.x();
        state.frame.sheet_top = frame.bounds.y() + room.top;
        state.frame.scrolled = Point::new(-ctx.snap(across), -ctx.snap(down));
        state.visible = visible;

        self.reread(state, ctx, &shown);
        let (laid, visible_rows) = self.bands(state, down, template.line_height);
        let look = Look::new(
            template,
            wrap_width,
            palette,
            ctx.theme().syntax(),
            state.coloured,
        );
        let syntax = ctx.theme().syntax();
        let EditingState {
            block,
            syntax: parsed,
            ..
        } = state;
        block.sync(
            &shown,
            Lines {
                laid,
                retained: visible_rows,
                template,
            },
            look,
            |shown, drawn| spans_of(parsed.as_mut(), shown, drawn, syntax, palette.text_muted),
        );
        let laid_widest = state.block.widest();
        state.widest = match self.options.single_line {
            true => laid_widest,
            false => state.widest.max(laid_widest),
        };

        let drawn = match state.block.laid_lines() {
            Some(lines) => shown.line_start(lines.start)..shown.line_range(lines.end - 1).end,
            None => 0..0,
        };
        let sheet = Sheet {
            shown: &shown,
            marking: self.found(state, ctx, &shown, drawn),
            selection: self.selection(state, ctx, &shown),
            animated: None,
            composition: shown
                .preedit()
                .map(|span| preedit::rules(&ctx.theme().palette(), span, shown.converting())),
            ink: ctx.animate_curve("ink", room.ink, motion::COLOR),
        };
        let sheet = Sheet {
            animated: self.animated_selection(state, ctx, &sheet),
            ..sheet
        };
        let caret = match ctx.focused() {
            true => self.caret(state, ctx, &shown, template.size),
            false => {
                state.blink.restart();
                state.caret = None;
                ctx.restart("caret_x");
                ctx.restart("caret_y");
                None
            }
        };
        let dropping = self.drop_caret(state, &shown, template.size);
        self.draw_sheet(state, ctx, &sheet, caret, dropping);
        self.draw_placeholder(state, ctx, &shown, template, room.hint);
        self.draw_bars(state, ctx, down, across);
        if self.assists() {
            self.draw_completion(state, ctx, &shown, template);
        }
        state.menu.paint(ctx);
    }

    fn fit(&self, state: &mut EditingState, room: &Room, template: &TextRun) -> Frame {
        let visible_rows = Size::new(room.right, room.top);
        let mut lanes = match self.options.bars {
            true => state.lanes,
            false => Lanes::default(),
        };
        let mut attempts = 0;
        let mut risen = 0;
        loop {
            let free = lanes.room(room.bounds, visible_rows);
            let area = Rect::new(
                free.x() + room.left,
                free.y(),
                (free.width() - room.left - room.right).max(0.0),
                free.height(),
            );
            let wrap_width = self.wraps().then_some(area.width());
            {
                let content = self.document.content();
                risen += state.rows.sync(
                    &self.shown(&content),
                    template,
                    wrap_width,
                    state.visible.clone(),
                );
            }
            let content = Size::new(
                match self.wraps() {
                    true => area.width(),
                    false => state.widest + room.left + room.right,
                },
                state.rows.total() as f32 * template.line_height + room.top * 2.0,
            );
            let needed_lanes = Lanes::needed(free, content);
            attempts += 1;
            if self.options.bars && needed_lanes != lanes && attempts < SETTLE {
                lanes = needed_lanes;
                continue;
            }
            state.lanes = lanes;
            state.down.shift(risen as f32 * template.line_height);
            return Frame {
                bounds: room.bounds,
                room: free,
                area,
                top: room.top,
                line_height: template.line_height,
                content_height: content.height,
                content_width: content.width,
                ..state.frame
            };
        }
    }

    fn stirred(&self, state: &mut EditingState) -> bool {
        let caret_position = {
            let content = self.document.content();
            let shown = self.shown(&content);
            CaretPosition {
                caret: shown.caret(),
                revision: shown.revision(),
                width: state.frame.area.width().to_bits(),
            }
        };
        state.caret_position.replace(caret_position) != Some(caret_position)
    }

    fn follow_caret(&self, state: &mut EditingState, template: &TextRun, wrap_width: Option<f32>) {
        state.follow = false;
        let frame = state.frame;
        let (line, row, x, reach) = {
            let content = self.document.content();
            let shown = self.shown(&content);
            let (row, x, reach) = walk::place_caret(&shown, template, wrap_width);
            (shown.line_of(shown.caret()), row, x, reach)
        };
        let top = (state.rows.before(line) + row) as f32 * frame.line_height + frame.top;
        state.down.aim(bar::follow(
            state.down.target(),
            top,
            frame.line_height,
            frame.room.height(),
            bar::max_scroll(Axis::Vertical, frame.room, frame.content_height),
        ));

        if self.wraps() {
            return;
        }
        let across = self.reaching(state, reach);
        let to = bar::follow(
            state.across.target(),
            x,
            caret::WIDTH,
            frame.area.width(),
            bar::max_scroll(Axis::Horizontal, frame.room, across),
        );
        match self.options.single_line {
            true => state.across.snap(to),
            false => state.across.aim(to),
        }
    }

    fn reaching(&self, state: &mut EditingState, reach: f32) -> f32 {
        let frame = state.frame;
        let inset = frame.bounds.width() - frame.area.width();
        let reached = reach + inset;
        (state.widest, state.frame.content_width) = match self.options.single_line {
            true => (reach, reached),
            false => (state.widest.max(reach), frame.content_width.max(reached)),
        };
        state.frame.content_width
    }

    fn first_row(down: f32, frame: &Frame, line_height: f32) -> u32 {
        ((down - frame.top) / line_height).floor().max(0.0) as u32
    }

    fn visible_lines(
        &self,
        state: &EditingState,
        down: f32,
        frame: &Frame,
        line_height: f32,
    ) -> Range<usize> {
        let first_row = Self::first_row(down, frame, line_height);
        let rows = (frame.room.height() / line_height).ceil() as u32 + 1;
        let (first, _) = state.rows.at(first_row);
        let (last, _) = state
            .rows
            .at(first_row.saturating_add(rows).saturating_sub(1));
        first..last.max(first) + 1
    }

    fn bands(
        &self,
        state: &EditingState,
        down: f32,
        line_height: f32,
    ) -> (Range<usize>, Range<usize>) {
        let frame = &state.frame;
        let first_row = Self::first_row(down, frame, line_height);
        let rows = (frame.room.height() / line_height).ceil() as u32 + 1;
        let lines = |pages: u32| {
            let reach = rows.saturating_mul(pages);
            let (first, _) = state.rows.at(first_row.saturating_sub(reach));
            let (last, _) = state
                .rows
                .at(first_row.saturating_add(rows + reach).saturating_sub(1));
            first..last.max(first) + 1
        };
        (lines(LAID_PAGES), lines(KEPT_PAGES))
    }

    fn reread(&self, state: &mut EditingState, ctx: &mut PaintCtx<'_>, shown: &DisplayText<'_>) {
        let read = self.reading(state, shown);
        if read == Some(false) {
            ctx.request_frame();
        }
        state.coloured = read == Some(true);
    }

    fn reading(&self, state: &mut EditingState, shown: &DisplayText<'_>) -> Option<bool> {
        let Some(language) = self.language else {
            state.syntax = None;
            return None;
        };
        if state
            .syntax
            .as_ref()
            .is_none_or(|parsed| parsed.language() != language)
        {
            state.syntax = rikka_syntax::Parsed::new(language);
        }
        Some(state.syntax.as_mut()?.read(shown.content(), PARSE_BUDGET))
    }

    fn selection(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        shown: &DisplayText<'_>,
    ) -> Option<(Range<usize>, Color)> {
        let selection = shown.content().selection();
        let hidden = self.options.single_line && !ctx.focus_within();
        let shown_range = (!selection.is_empty() && !hidden).then(|| shown.range(selection));
        let tint = shown_range
            .as_ref()
            .map(|_| ctx.theme().palette().selection);
        let Some(fill) = ctx.fade("selection_fill", tint, motion::HOVER) else {
            state.selecting = false;
            return None;
        };
        Some((shown_range.unwrap_or(0..0), fill))
    }

    fn animated_selection(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        sheet: &Sheet<'_>,
    ) -> Option<(Vec<Rect>, Color)> {
        let (range, fill) = sheet.selection.clone()?;
        if !self.options.single_line {
            state.selecting = false;
            return None;
        }
        let frame = state.frame;
        let bands: Vec<Rect> = match range.is_empty() {
            true => Vec::new(),
            false => state
                .block
                .selection(sheet.shown, &state.rows, range)
                .into_iter()
                .map(|band| frame.placed(band))
                .collect(),
        };
        let home = frame.bounds.origin;
        let away = Point::ZERO - home;
        let drawn = match bands.as_slice() {
            [band] => {
                if !state.selecting {
                    ctx.jump("selection", band.translate(away));
                }
                state.selecting = true;
                let band = ctx.animate_curve("selection", band.translate(away), motion::CARET);
                vec![band.translate(home)]
            }
            [] => {
                state.selecting = false;
                ctx.hold::<Rect>("selection", None, motion::CARET)
                    .into_iter()
                    .map(|band| band.translate(home))
                    .collect()
            }
            _ => {
                state.selecting = false;
                if let Some(first) = bands.first() {
                    ctx.jump("selection", first.translate(away));
                }
                bands
            }
        };
        Some((drawn, fill))
    }

    fn draw_sheet(
        &self,
        state: &EditingState,
        ctx: &mut PaintCtx<'_>,
        sheet: &Sheet<'_>,
        caret: Option<(Rect, Color)>,
        dropping: Option<Rect>,
    ) {
        let frame = state.frame;
        let wraps = self.wraps();
        ctx.clipped(frame.area, |ctx| {
            ctx.scrolled(frame.scrolled, |ctx| {
                for (index, piece) in state.block.pieces().iter().enumerate() {
                    let top = Point::new(frame.origin, frame.sheet_top + piece.top(&state.rows));
                    ctx.part(piece.first(), |ctx| {
                        if let Some(marking) = &sheet.marking {
                            state.finding.draw(ctx, sheet.shown, piece, marking, top);
                        }
                        match &sheet.animated {
                            Some((bands, fill)) if index == 0 => {
                                for band in bands {
                                    ctx.draw_quad(
                                        Quad::new(*band)
                                            .fill(*fill)
                                            .radius(caret::SELECTION_RADIUS),
                                    );
                                }
                            }
                            Some(_) => {}
                            None => {
                                if let Some((range, fill)) = &sheet.selection {
                                    draw_bands(ctx, piece, sheet.shown, range.clone(), top, *fill);
                                }
                            }
                        }
                        let width = match wraps {
                            true => frame.area.width(),
                            false => piece.laid().widest().max(frame.area.width()),
                        };
                        let height = piece.rows() as f32 * piece.run().line_height + SLACK;
                        ctx.draw_laid_text(
                            Rect::new(top.x, top.y, width, height),
                            piece.run().clone(),
                            piece.laid().clone(),
                            sheet.ink,
                        );
                        if let Some(rules) = &sheet.composition {
                            draw_rules(ctx, piece, sheet.shown, rules, top);
                        }
                    });
                }
                if let Some((bar, fill)) = caret {
                    ctx.draw_quad(Quad::new(bar).fill(fill).radius(caret::WIDTH / 2.0));
                }
                if let Some(bar) = dropping {
                    let fill = ctx.theme().palette().accent;
                    ctx.draw_quad(Quad::new(bar).fill(fill).radius(caret::WIDTH));
                }
            });
        });
    }

    fn parked_caret(state: &EditingState, shown: &DisplayText<'_>, at: usize, size: f32) -> Rect {
        let area = state.frame.area;
        let height = size * caret::HEIGHT;
        let y = match shown.line_of(at) < state.visible.start {
            true => area.y(),
            false => area.max_y() - height,
        };
        Rect::new(area.x(), y, caret::WIDTH, height)
    }

    fn caret(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        shown: &DisplayText<'_>,
        size: f32,
    ) -> Option<(Rect, Color)> {
        let at = shown.caret();
        let Some((x, y)) = state.block.caret(shown, &state.rows, at, shown.affinity()) else {
            state.caret = Some(Self::parked_caret(state, shown, at, size));
            return None;
        };
        let blinking_at = (at, shown.revision());
        if state.blinking_at != Some(blinking_at) {
            state.blinking_at = Some(blinking_at);
            state.blink.restart();
        }
        let frame = state.frame;
        let (x, y) = (frame.origin + x, frame.sheet_top + y);
        let height = size * caret::HEIGHT;
        state.caret = Some(match state.visible.contains(&shown.line_of(at)) {
            true => frame.on_screen(Rect::new(
                x,
                y + (frame.line_height - height) / 2.0,
                caret::WIDTH,
                height,
            )),
            false => Self::parked_caret(state, shown, at, size),
        });
        let home = frame.bounds.origin;
        let x = home.x + ctx.animate_curve("caret_x", x - home.x, motion::CARET);
        let y = home.y + ctx.animate_curve("caret_y", y - home.y, motion::CARET);
        let bar = Rect::new(
            x - caret::WIDTH / 2.0,
            y + (frame.line_height - height) / 2.0,
            caret::WIDTH,
            height,
        );
        let alpha = state.blink.alpha(ctx);
        (alpha > 0.0).then(|| (bar, ctx.theme().palette().accent.scale_alpha(alpha)))
    }

    fn drop_caret(&self, state: &EditingState, shown: &DisplayText<'_>, size: f32) -> Option<Rect> {
        let at = state.drop_at?;
        let (x, y) = state
            .block
            .caret(shown, &state.rows, shown.offset(at), Affinity::After)?;
        let frame = state.frame;
        let height = size * caret::HEIGHT;
        Some(Rect::new(
            frame.origin + x - caret::WIDTH,
            frame.sheet_top + y + (frame.line_height - height) / 2.0,
            caret::WIDTH * 2.0,
            height,
        ))
    }

    fn draw_placeholder(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        shown: &DisplayText<'_>,
        template: &TextRun,
        color: Color,
    ) {
        let Some(placeholder) = self.placeholder.as_ref() else {
            return;
        };
        let target = shown.is_empty().then_some(color);
        let Some(color) = ctx.fade("placeholder", target, motion::COLOR) else {
            return;
        };
        let area = state.frame.area;
        let hint = TextRun {
            content: placeholder.as_str().into(),
            wrap: Wrap::None,
            ..template.clone()
        };
        let within = Rect::new(
            area.x(),
            state.frame.sheet_top,
            area.width(),
            template.line_height,
        );
        draw_cell(ctx, within, area.x(), area.width(), hint, color);
    }

    fn draw_bars(&self, state: &mut EditingState, ctx: &mut PaintCtx<'_>, down: f32, across: f32) {
        if !self.options.bars {
            return;
        }
        let frame = state.frame;
        let attentive = ctx.hovered();
        let drag_origin = self.holds_bar(state);
        bar::paint_thumb(
            ctx,
            Axis::Vertical,
            bar::track(Axis::Vertical, frame.bounds, frame.room),
            frame.content_height,
            down,
            (attentive, drag_origin),
        );
        bar::paint_thumb(
            ctx,
            Axis::Horizontal,
            bar::track(Axis::Horizontal, frame.bounds, frame.room),
            frame.content_width,
            across,
            (attentive, drag_origin),
        );
    }
}

fn draw_bands(
    ctx: &mut PaintCtx<'_>,
    piece: &Piece,
    shown: &DisplayText<'_>,
    range: Range<usize>,
    top: Point,
    fill: Color,
) {
    if range.is_empty() {
        return;
    }
    for band in piece.selection(shown, range) {
        ctx.draw_quad(
            Quad::new(band.translate(top))
                .fill(fill)
                .radius(caret::SELECTION_RADIUS),
        );
    }
}

fn draw_rules(
    ctx: &mut PaintCtx<'_>,
    piece: &Piece,
    shown: &DisplayText<'_>,
    rules: &[(Range<usize>, f32, Color); 2],
    top: Point,
) {
    for (range, thickness, color) in rules {
        if range.is_empty() {
            continue;
        }
        for band in piece.selection(shown, range.clone()) {
            let band = band.translate(top);
            let rule = Rect::new(band.x(), band.max_y() - thickness, band.width(), *thickness);
            ctx.draw_quad(Quad::new(rule).fill(*color));
        }
    }
}

fn spans_of(
    parsed: Option<&mut rikka_syntax::Parsed>,
    shown: &DisplayText<'_>,
    drawn: Range<usize>,
    syntax: Syntax,
    hint: Color,
) -> Vec<TextSpan> {
    let mut spans = match parsed {
        Some(parsed) => {
            let within = shown.document_offset(drawn.start)..shown.document_offset(drawn.end);
            let mut spans = parsed.spans(shown.content(), within, syntax);
            for span in &mut spans {
                let at = |offset: usize| shown.offset(offset).saturating_sub(drawn.start);
                span.range = at(span.range.start)..at(span.range.end);
            }
            spans
        }
        None => Vec::new(),
    };
    if let Some(ghost) = shown
        .ghost()
        .filter(|ghost| ghost.start >= drawn.start && ghost.end <= drawn.end)
    {
        spans = ghosted(
            spans,
            ghost.start - drawn.start..ghost.end - drawn.start,
            hint,
        );
    }
    spans
}

fn ghosted(spans: Vec<TextSpan>, ghost: Range<usize>, hint: Color) -> Vec<TextSpan> {
    let mut carved: Vec<TextSpan> = Vec::with_capacity(spans.len() + 2);
    for span in spans {
        if span.range.end <= ghost.start || span.range.start >= ghost.end {
            carved.push(span);
            continue;
        }
        if span.range.start < ghost.start {
            let mut before = span.clone();
            before.range = span.range.start..ghost.start;
            carved.push(before);
        }
        if span.range.end > ghost.end {
            let mut after = span;
            after.range = ghost.end..after.range.end;
            carved.push(after);
        }
    }
    carved.push(TextSpan::new(ghost).color(hint));
    carved.sort_by_key(|span| span.range.start);
    carved
}
