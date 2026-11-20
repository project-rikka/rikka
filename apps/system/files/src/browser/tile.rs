use std::collections::HashSet;
use std::rc::Rc;

use files_core::ViewMode;
use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{Command, DRAG_SLOP, Element};

use crate::browser::layout::{Column, ROW, TILE, TILE_ART, TILE_LINE, TILE_LINES};
use crate::browser::{Browser, GUTTER, Rename, faded_tint, tint};

const FIELD: ControlSize = ControlSize::Small;

pub(crate) struct Cell {
    pub(crate) browser: Rc<Browser>,
    pub(crate) index: usize,
}

#[derive(Debug, Default)]
pub(crate) struct CellState {
    pressed: bool,
    pending_drag: Option<PendingDrag>,
}

#[derive(Clone, Copy, Debug)]
struct PendingDrag {
    at: Point,
    collapse: bool,
}

impl Cell {
    pub(crate) fn element(self, height: f32) -> Element {
        let width = match self.browser.mode {
            ViewMode::Details => Length::FILL,
            ViewMode::Icons => Length::px(TILE),
        };
        let field = self.renaming().map(|rename| self.field(rename));
        let cell = match field {
            Some(field) => Element::with_children(self, vec![field]),
            None => Element::new(self),
        };
        cell.width(width).height(height)
    }

    fn renaming(&self) -> Option<&Rename> {
        self.browser
            .rename
            .as_ref()
            .filter(|rename| rename.index == self.index)
    }

    fn editing(&self) -> bool {
        self.renaming().is_some()
    }

    fn field(&self, rename: &Rename) -> Element {
        let frame = self.field_frame();
        let font = match self.browser.mode {
            ViewMode::Details => 13.0,
            ViewMode::Icons => 12.0,
        };
        let mut field = text_field(&rename.document)
            .control_size(FIELD)
            .font_size(font)
            .claims_focus(true);
        if let Some(on_rename) = self.browser.on_rename.clone() {
            let document = rename.document.clone();
            let submit = on_rename.map(move |(): ()| (document.text(),));
            field = field.on_submit(submit.clone()).on_blur(submit);
        }
        field
            .position(frame.origin.x, frame.origin.y)
            .width(frame.size.width)
            .height(frame.size.height)
            .build()
            .keyed(rename.session)
    }

    fn field_frame(&self) -> Rect {
        match self.browser.mode {
            ViewMode::Details => {
                let left = COLUMN_PADDING + GUTTER - FIELD.text_inset();
                let right = self.browser.widths.at(0) - COLUMN_PADDING;
                Rect::new(
                    left,
                    2.0,
                    (right - left).max(Column::Name.floor()),
                    ROW - 4.0,
                )
            }
            ViewMode::Icons => Rect::new(0.0, TILE_ART + 8.0, TILE, TILE_LINE + 8.0),
        }
    }

    fn paint_row(&self, ctx: &mut PaintCtx<'_>) {
        let Some(row) = self.browser.rows.get(self.index) else {
            return;
        };
        let palette = palette();
        let line = ctx.bounds();
        let widths = &self.browser.widths;
        let metrics = TextRun::new("", 13.0).no_wrap();
        let selected = self.browser.chosen.contains(&self.index);

        if selected {
            ctx.draw_quad(Quad::new(line).fill(palette.selection));
        } else if self.index % 2 == 1 {
            let mut stripe = palette.surface_variant;
            stripe.a *= 0.45;
            ctx.draw_quad(Quad::new(line).fill(stripe));
        }
        if self.browser.focus == Some(self.index) && ctx.focus_within() {
            ctx.draw_quad(
                Quad::new(line)
                    .fill(Color::TRANSPARENT)
                    .stroke(1.0, palette.outline),
            );
        }

        let colour = match row.hidden {
            true => palette.text_muted,
            false => palette.ink(selected),
        };
        let icon = Rect::new(
            line.origin.x + COLUMN_PADDING,
            line.origin.y + 4.0,
            20.0,
            20.0,
        );
        ctx.draw_icon(
            icon,
            IconGlyph::from(row.symbol()).spec(18.0),
            tint(row, &palette),
        );

        for (column, which) in Column::ALL.into_iter().enumerate() {
            let text = row.cell(which);
            if text.is_empty() || (column == 0 && self.editing()) {
                continue;
            }
            let (start, width) = (widths.start(column), widths.at(column));
            let gutter = match column == 0 {
                true => GUTTER,
                false => 0.0,
            };
            let room = width - COLUMN_PADDING * 2.0 - gutter;
            let shown = fitted_line(&metrics.clone().content(text), room);
            let x = match which.right_aligned() {
                true => line.origin.x + start + width - COLUMN_PADDING - shown.width,
                false => line.origin.x + start + COLUMN_PADDING + gutter,
            };
            let at = Rect::new(
                x,
                line.origin.y + (ROW - 17.0) / 2.0,
                shown.width + 2.0,
                17.0,
            );
            let tone = match column == 0 {
                true => colour,
                false => palette.text_muted,
            };
            ctx.draw_text(at, TextRun::new(shown.text.as_str(), 13.0), tone);
        }
    }

    fn paint_tile(&self, ctx: &mut PaintCtx<'_>) {
        let Some(row) = self.browser.rows.get(self.index) else {
            return;
        };
        let palette = palette();
        let tile = ctx.bounds();
        let selected = self.browser.chosen.contains(&self.index);

        if selected {
            ctx.draw_quad(
                Quad::new(tile)
                    .fill(palette.selection)
                    .radius(Corner::Item)
                    .stroke(1.0, palette.accent),
            );
        } else if self.browser.focus == Some(self.index) && ctx.focus_within() {
            ctx.draw_quad(
                Quad::new(tile)
                    .fill(Color::TRANSPARENT)
                    .radius(Corner::Item)
                    .stroke(1.0, palette.outline),
            );
        }

        let art = Rect::new(tile.origin.x, tile.origin.y + 6.0, TILE, TILE_ART);
        {
            let thumbnails = self.browser.thumbnails.borrow();
            match thumbnails.get(self.index).and_then(Option::as_ref) {
                Some(thumbnail) => ctx.draw_image(
                    ImageFit::ScaleDown.place(art.inflate(-6.0), thumbnail.pixels()),
                    thumbnail.shown(),
                ),
                None => ctx.draw_icon(
                    art,
                    IconGlyph::from(row.symbol()).spec(52.0),
                    faded_tint(row, &palette),
                ),
            }
        }
        if row.linked {
            let badge = Rect::new(art.min_x() + 8.0, art.max_y() - 20.0, 18.0, 18.0);
            ctx.draw_quad(Quad::new(badge).fill(palette.surface).radius(Corner::Item));
            ctx.draw_icon(
                badge,
                IconGlyph::from(MaterialSymbol::LINK).spec(13.0),
                palette.text_muted,
            );
        }

        if self.editing() {
            return;
        }
        let metrics = TextRun::new("", 12.0).no_wrap();
        let wrapped = fitted_lines(
            &metrics.clone().content(row.name.as_str()),
            TILE - 8.0,
            TILE_LINES,
        );
        let colour = match row.hidden {
            true => palette.text_muted,
            false => palette.ink(selected),
        };
        for (line, fitted) in wrapped.iter().enumerate() {
            let at = Rect::new(
                tile.origin.x + (TILE - fitted.width) / 2.0,
                art.max_y() + 4.0 + line as f32 * TILE_LINE,
                fitted.width + 2.0,
                TILE_LINE,
            );
            ctx.draw_text(at, TextRun::new(fitted.text.as_str(), 12.0), colour);
        }
    }

    fn drag_started(&self, state: &mut CellState, event: &Input, ctx: &mut EventCtx<'_>) -> bool {
        let Some(pending_drag) = state.pending_drag else {
            return false;
        };
        match *event {
            Input::PointerMoved(position) => {
                ctx.set_handled();
                if position.distance(pending_drag.at) > DRAG_SLOP {
                    state.pending_drag = None;
                    ctx.release_pointer();
                    self.browser.start_drag(self.index, ctx);
                }
                true
            }
            Input::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                state.pending_drag = None;
                ctx.release_pointer();
                ctx.set_handled();
                if pending_drag.collapse {
                    self.browser
                        .select(HashSet::from([self.index]), Some(self.index), ctx);
                }
                true
            }
            _ => false,
        }
    }

    fn paint_aim(&self, ctx: &mut PaintCtx<'_>) {
        if !self.browser.is_drop_target(self.index) {
            return;
        }
        let accent = palette().accent;
        let radius = match self.browser.mode {
            ViewMode::Details => 4.0,
            ViewMode::Icons => 8.0,
        };
        ctx.draw_quad(
            Quad::new(ctx.bounds())
                .fill(accent.scale_alpha(0.16))
                .stroke(2.0, accent)
                .radius(radius),
        );
    }

    fn edited(&self, event: &Input, ctx: &mut EventCtx<'_>) {
        match *event {
            Input::KeyPressed {
                key: Key::Escape, ..
            } => {
                ctx.set_handled();
                self.browser.rename_cancel(ctx);
            }
            Input::KeyPressed { key: Key::Tab, .. } => {
                ctx.set_handled();
                self.browser.finish_rename(ctx);
            }
            Input::KeyPressed { key, modifiers, .. } => {
                if Command::for_key(key, modifiers).is_none() {
                    ctx.set_handled();
                }
            }
            Input::PointerPressed { position, .. } => {
                ctx.set_handled();
                let field = self.field_frame().translate(ctx.bounds().origin);
                if !field.contains(position) {
                    self.browser.finish_rename(ctx);
                }
            }
            _ => {}
        }
    }
}

impl Widget for Cell {
    type State = CellState;

    fn paint(&self, _state: &mut CellState, ctx: &mut PaintCtx<'_>) {
        self.browser.drawing.watch(ctx.handle());
        match self.browser.mode {
            ViewMode::Details => self.paint_row(ctx),
            ViewMode::Icons => self.paint_tile(ctx),
        }
        self.paint_aim(ctx);
    }

    fn event(&self, state: &mut CellState, event: &Input, ctx: &mut EventCtx<'_>) {
        if self.editing() {
            self.edited(event, ctx);
            return;
        }
        if self.drag_started(state, event, ctx) {
            return;
        }
        let Input::PointerPressed {
            button: button @ (PointerButton::Primary | PointerButton::Middle),
            position,
            modifiers,
        } = *event
        else {
            return;
        };
        self.browser.finish_rename(ctx);
        ctx.request_focus();
        ctx.set_handled();

        if button == PointerButton::Middle {
            self.browser.open_tab(self.index, ctx);
            return;
        }

        let twice = ctx.presses() == 2 && state.pressed && !modifiers.control && !modifiers.shift;
        state.pressed = true;
        if twice {
            self.browser.open(self.index, ctx);
            return;
        }
        let collapse =
            self.browser.chosen.contains(&self.index) && !modifiers.control && !modifiers.shift;
        if !collapse {
            let chosen = self.browser.after_click(self.index, modifiers);
            self.browser.select(chosen, Some(self.index), ctx);
        }
        if !modifiers.control {
            state.pending_drag = Some(PendingDrag {
                at: position,
                collapse,
            });
            ctx.capture_pointer();
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::TAKES_TAB.when(self.editing())
    }
}
