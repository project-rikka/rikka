use rikka_color_formats::{Rgb, Template};
use rikka_settings::session::color_picker::Button;
use rikka_types::{CursorShape, Key, Modifiers, Point, PointerButton, Rect};
use rikka_ui::{Behavior, Element, Event, EventCtx, Layout, Length, PaintCtx, View, Widget};

use crate::picker::card::Card;
use crate::picker::frame::Frame;
use crate::picker::loupe::Loupe;
use crate::picker::sheet::Sheet;

const CELLS: [f32; 8] = [4.0, 6.0, 8.0, 10.0, 13.0, 16.0, 20.0, 26.0];
const FIRST_CELL: usize = 3;
const LEAP: i64 = 10;

pub(crate) struct Stage {
    frame: Option<Frame>,
    template: Template,
    names: bool,
}

#[must_use]
pub(crate) fn stage(frame: Option<Frame>, template: Template, names: bool) -> Stage {
    Stage {
        frame,
        template,
        names,
    }
}

pub(crate) struct StageState {
    pointer: Option<Point>,
    nudge: (i64, i64),
    zoom: usize,
    rolled: f32,
    pressed_button: Option<PointerButton>,
}

impl Default for StageState {
    fn default() -> StageState {
        StageState {
            pointer: None,
            nudge: (0, 0),
            zoom: FIRST_CELL,
            rolled: 0.0,
            pressed_button: None,
        }
    }
}

fn surrounding(room: Rect, hole: Rect) -> impl Iterator<Item = Rect> {
    [
        Rect::new(
            room.min_x(),
            room.min_y(),
            room.width(),
            hole.min_y() - room.min_y(),
        ),
        Rect::new(
            room.min_x(),
            hole.max_y(),
            room.width(),
            room.max_y() - hole.max_y(),
        ),
        Rect::new(
            room.min_x(),
            hole.min_y(),
            hole.min_x() - room.min_x(),
            hole.height(),
        ),
        Rect::new(
            hole.max_x(),
            hole.min_y(),
            room.max_x() - hole.max_x(),
            hole.height(),
        ),
    ]
    .into_iter()
    .filter(|band| band.width() > 0.0 && band.height() > 0.0)
}

fn pressed(button: PointerButton) -> Option<Button> {
    match button {
        PointerButton::Primary => Some(Button::Primary),
        PointerButton::Middle => Some(Button::Middle),
        PointerButton::Secondary => Some(Button::Secondary),
        _ => None,
    }
}

impl Stage {
    fn pixel(&self, state: &StageState, bounds: Rect) -> Option<(u32, u32)> {
        let frame = self.frame.as_ref()?;
        let pointer = state.pointer?;
        if bounds.width() < 1.0 || bounds.height() < 1.0 || frame.width == 0 || frame.height == 0 {
            return None;
        }
        let across = |along: f32, start: f32, length: f32, pixels: u32, nudge: i64| {
            let scaled = ((along - start) * pixels as f32 / length).floor() as i64;
            (scaled + nudge).clamp(0, i64::from(pixels) - 1) as u32
        };
        Some((
            across(
                pointer.x,
                bounds.min_x(),
                bounds.width(),
                frame.width,
                state.nudge.0,
            ),
            across(
                pointer.y,
                bounds.min_y(),
                bounds.height(),
                frame.height,
                state.nudge.1,
            ),
        ))
    }

    fn aimed(&self, state: &StageState, bounds: Rect) -> Option<Rgb> {
        let (x, y) = self.pixel(state, bounds)?;
        Some(self.frame.as_ref()?.at(x, y))
    }

    fn pick(&self, state: &StageState, button: Button, ctx: &mut EventCtx<'_>) {
        let Some(color) = self.aimed(state, ctx.bounds()) else {
            return;
        };
        ctx.run(move |sheet: &mut Sheet| sheet.chose(color, button));
    }

    fn zoom(state: &mut StageState, by: isize, ctx: &mut EventCtx<'_>) {
        let zoom = state.zoom.saturating_add_signed(by).min(CELLS.len() - 1);
        if zoom != state.zoom {
            state.zoom = zoom;
            ctx.request_paint();
        }
    }

    fn nudge(state: &mut StageState, by: (i64, i64), ctx: &mut EventCtx<'_>) {
        if state.pointer.is_none() {
            return;
        }
        state.nudge = (state.nudge.0 + by.0, state.nudge.1 + by.1);
        ctx.request_paint();
    }

    fn keyed(
        &self,
        state: &mut StageState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let step = match modifiers.shift {
            true => LEAP,
            false => 1,
        };
        match key {
            Key::Escape => {
                ctx.run(Sheet::cancelled);
            }
            Key::Enter | Key::Space => self.pick(state, Button::Primary, ctx),
            Key::ArrowLeft => Stage::nudge(state, (-step, 0), ctx),
            Key::ArrowRight => Stage::nudge(state, (step, 0), ctx),
            Key::ArrowUp => Stage::nudge(state, (0, -step), ctx),
            Key::ArrowDown => Stage::nudge(state, (0, step), ctx),
            Key::Char('+' | '=') => Stage::zoom(state, 1, ctx),
            Key::Char('-') => Stage::zoom(state, -1, ctx),
            _ => return,
        }
        ctx.set_handled();
    }
}

impl Widget for Stage {
    type State = StageState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut StageState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let Some(frame) = &self.frame else {
            return;
        };
        let (Some(pointer), Some(pixel)) = (state.pointer, self.pixel(state, bounds)) else {
            ctx.draw_image(bounds, frame.picture.clone());
            return;
        };
        let rgb = frame.at(pixel.0, pixel.1);
        let loupe = Loupe::aimed(ctx, pixel, pointer, CELLS[state.zoom]);
        let name = self.names.then(|| rgb.name().label().as_str().to_owned());
        let card = Card::beside(ctx, loupe.projection, rgb, self.template.render(rgb), name);
        for around in surrounding(bounds, card.rect) {
            ctx.clipped(around, |ctx| ctx.draw_image(bounds, frame.picture.clone()));
        }
        loupe.paint(ctx, frame);
        card.paint(ctx);
    }

    fn event(&self, state: &mut StageState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerMoved(position) => {
                state.pointer = Some(*position);
                state.nudge = (0, 0);
                ctx.request_paint();
            }
            Event::PointerLeft => {
                state.pointer = None;
                ctx.request_paint();
            }
            Event::PointerPressed {
                position, button, ..
            } if pressed(*button).is_some() => {
                state.pointer = Some(*position);
                state.pressed_button = Some(*button);
                ctx.capture_pointer();
                ctx.set_handled();
                ctx.request_paint();
            }
            Event::PointerReleased { button, .. } if state.pressed_button == Some(*button) => {
                state.pressed_button = None;
                ctx.release_pointer();
                ctx.set_handled();
                if let Some(pressed) = pressed(*button) {
                    self.pick(state, pressed, ctx);
                }
            }
            Event::Scroll { delta, .. } => {
                state.rolled += delta.zoom();
                while state.rolled >= 1.0 {
                    state.rolled -= 1.0;
                    Stage::zoom(state, 1, ctx);
                }
                while state.rolled <= -1.0 {
                    state.rolled += 1.0;
                    Stage::zoom(state, -1, ctx);
                }
                ctx.set_handled();
            }
            Event::KeyPressed { key, modifiers, .. } => {
                self.keyed(state, *key, *modifiers, ctx);
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::Hidden)
    }

    fn behavior(&self, _state: &StageState) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }

    fn debug_name(&self) -> &'static str {
        "ColorPickerStage"
    }
}

impl View for Stage {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}
