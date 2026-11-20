use rikka_symbols::MaterialSymbol;
use rikka_syntax::Kind;
use rikka_text::{Affinity, IconSpec, TextSpan};
use rikka_theme::Corner;
use rikka_types::{Point, PointerButton, Rect, Size};
use rikka_ui::text::{DisplayText, TextRun};
use rikka_ui::{Event, EventCtx, PaintCtx, Quad, drop_panel, font, measure_text, motion};

use crate::rows::draw_cell;
use crate::text::editing::Editing;
use crate::text::editing::assist::complete::{Completing, SHOWN};
use crate::text::editing::state::EditingState;

const PAD: f32 = 4.0;
const GAP: f32 = 2.0;
const INSET: f32 = 8.0;
const ICON_GAP: f32 = 6.0;
const NARROWEST: f32 = 200.0;
const WIDEST: f32 = 440.0;
const ROW_SPACE: f32 = 1.45;
const LINE: f32 = 1.3;
const THUMB: f32 = 3.0;

const SHOWING: &str = "completion.showing";

fn symbol(kind: Kind) -> MaterialSymbol {
    match kind {
        Kind::Type => MaterialSymbol::DATA_OBJECT,
        Kind::Function => MaterialSymbol::FUNCTIONS,
        Kind::Module => MaterialSymbol::INVENTORY_2,
        Kind::Constant => MaterialSymbol::PIN,
        Kind::Field => MaterialSymbol::LABEL,
        Kind::Variable => MaterialSymbol::VARIABLES,
        Kind::Keyword => MaterialSymbol::KEY,
        Kind::Word => MaterialSymbol::ABC,
    }
}

fn row_height(template: &TextRun) -> f32 {
    (template.size * ROW_SPACE).round()
}

impl Editing {
    pub(crate) fn completion_pointer(
        &self,
        state: &mut EditingState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(completing) = state.completing.as_mut() else {
            return false;
        };
        let Some(panel) = completing.panel else {
            return false;
        };
        let panel = panel.translate(ctx.bounds().origin);
        let row = panel.height() - PAD * 2.0;
        let rows = completing.offered.len().clamp(1, SHOWN);
        let height = row / rows as f32;
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if panel.contains(position) => {
                let index = ((position.y - panel.y() - PAD) / height).floor().max(0.0) as usize;
                completing.chosen = (completing.top + index).min(completing.offered.len() - 1);
                ctx.set_handled();
                self.accept(state, ctx);
                true
            }
            Event::PointerPressed { position, .. } if !panel.contains(position) => {
                self.dismiss(state, ctx);
                false
            }
            Event::Scroll {
                delta, position, ..
            } if panel.contains(position) => {
                let down = delta.offset().y;
                let rows = match down.abs() < height {
                    true => down.signum() as isize,
                    false => (down / height).round() as isize,
                };
                completing.scroll(rows);
                ctx.set_handled();
                ctx.request_paint();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn draw_completion(
        &self,
        state: &mut EditingState,
        ctx: &mut PaintCtx<'_>,
        shown: &DisplayText<'_>,
        template: &TextRun,
    ) {
        let frame = state.frame;
        let block = &state.block;
        let rows = &state.rows;
        let Some(completing) = state.completing.as_mut() else {
            return;
        };
        completing.panel = None;
        if !ctx.focused() || completing.offered.is_empty() {
            return;
        }
        let Some((x, y)) =
            block.caret(shown, rows, shown.offset(completing.start), Affinity::After)
        else {
            return;
        };
        let anchor = frame.on_screen(Rect::new(
            frame.origin + x - INSET - template.size - ICON_GAP,
            frame.sheet_top + y,
            1.0,
            frame.line_height,
        ));
        let row = row_height(template);
        let width = match completing.width {
            Some(width) => width,
            None => {
                let width = widest(completing, template);
                completing.width = Some(width);
                width
            }
        };
        let count = completing.offered.len().min(SHOWN);
        let size = Size::new(width, count as f32 * row + PAD * 2.0);
        let dropped = drop_panel(anchor, GAP, size, ctx.viewport());
        let rect = dropped.rect();
        completing.panel = Some(rect.translate(Point::ZERO - ctx.bounds().origin));
        if std::mem::take(&mut completing.unseen) {
            ctx.restart(SHOWING);
        }
        let shown = ctx.progress(SHOWING, true, motion::POP, motion::POP_OUT);
        let completing = &*completing;
        ctx.topmost(|ctx| {
            ctx.popped(shown, dropped.pivot(), |ctx| {
                paint_panel(ctx, completing, rect, row, template);
            });
        });
    }
}

fn label_run(word: &str, template: &TextRun) -> TextRun {
    let mut run = TextRun::new(word, template.size).no_wrap();
    run.family = template.family.clone();
    run.line_height = (template.size * LINE).round();
    run
}

fn widest(completing: &Completing, template: &TextRun) -> f32 {
    let longest = completing
        .offered
        .iter()
        .map(|suggestion| measure_text(&label_run(suggestion.word(), template), None).width)
        .fold(0.0f32, f32::max);
    (longest + template.size + ICON_GAP + INSET * 2.0 + PAD * 2.0).clamp(NARROWEST, WIDEST)
}

fn paint_panel(
    ctx: &mut PaintCtx<'_>,
    completing: &Completing,
    rect: Rect,
    row: f32,
    template: &TextRun,
) {
    let palette = ctx.theme().palette();
    let syntax = ctx.theme().syntax();
    ctx.draw_pane(
        Quad::new(rect)
            .fill(palette.surface)
            .stroke(1.0, palette.outline)
            .radius(Corner::Item),
    );
    let shown = completing
        .offered
        .iter()
        .enumerate()
        .skip(completing.top)
        .take(SHOWN);
    for (slot, (index, suggestion)) in shown.enumerate() {
        let band = Rect::new(
            rect.x() + PAD,
            rect.y() + PAD + slot as f32 * row,
            rect.width() - PAD * 2.0,
            row,
        );
        let chosen = index == completing.chosen;
        if chosen {
            ctx.draw_quad(
                Quad::new(band)
                    .fill(palette.selection)
                    .radius(Corner::Detail),
            );
        }
        let ink = match chosen {
            true => palette.on_selection,
            false => palette.text,
        };
        let tint = suggestion
            .kind()
            .token()
            .map_or(palette.text_muted, |token| syntax.color(token));
        let glyph = Rect::new(
            band.x() + INSET,
            band.center().y - template.size / 2.0,
            template.size,
            template.size,
        );
        ctx.draw_icon(
            glyph,
            IconSpec::new(symbol(suggestion.kind()).codepoint(), template.size),
            tint,
        );
        let mut run = label_run(suggestion.word(), template);
        run.spans = suggestion
            .matched()
            .iter()
            .map(|range| {
                TextSpan::new(range.clone())
                    .color(palette.accent)
                    .weight(font::BOLD)
            })
            .collect::<Vec<_>>()
            .into();
        let x = glyph.max_x() + ICON_GAP;
        draw_cell(ctx, band, x, (band.max_x() - INSET - x).max(0.0), run, ink);
    }
    if completing.offered.len() > SHOWN {
        let track = rect.height() - PAD * 2.0;
        let share = SHOWN as f32 / completing.offered.len() as f32;
        let travel = completing.top as f32 / (completing.offered.len() - SHOWN) as f32;
        let height = (track * share).max(THUMB * 4.0);
        let thumb = Rect::new(
            rect.max_x() - PAD / 2.0 - THUMB,
            rect.y() + PAD + (track - height) * travel,
            THUMB,
            height,
        );
        ctx.draw_quad(
            Quad::new(thumb)
                .fill(palette.text_muted.scale_alpha(0.4))
                .radius(THUMB / 2.0),
        );
    }
}
