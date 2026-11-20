use rikka_app::prelude::*;
use rikka_input::Chord;
use rikka_ui::Feed;

use crate::project::act::act;
use crate::project::machine_input::MachineInput;

pub(super) struct Live {
    feed: Feed,
    shot: (u32, u32),
    taking: bool,
}

impl Live {
    pub(super) fn new(feed: Feed, shot: (u32, u32), taking: bool) -> Live {
        Live { feed, shot, taking }
    }

    fn screen(&self, bounds: Rect) -> Option<Rect> {
        let shown = self.feed.size()?;
        let (width, height) = (shown.width, shown.height);
        if width <= 0.0 || height <= 0.0 || bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return None;
        }

        let scale = (bounds.width() / width).min(bounds.height() / height);
        let (across, down) = (width * scale, height * scale);
        Some(Rect::new(
            bounds.x() + (bounds.width() - across) / 2.0,
            bounds.y() + (bounds.height() - down) / 2.0,
            across,
            down,
        ))
    }

    fn place(&self, bounds: Rect, at: Point) -> Option<(u32, u32)> {
        let (place, on) = self.nearest(bounds, at)?;
        on.then_some(place)
    }

    fn edged(&self, bounds: Rect, at: Point) -> Option<(u32, u32)> {
        Some(self.nearest(bounds, at)?.0)
    }

    fn nearest(&self, bounds: Rect, at: Point) -> Option<((u32, u32), bool)> {
        let screen = self.screen(bounds)?;
        let across = (at.x - screen.x()) / screen.width();
        let down = (at.y - screen.y()) / screen.height();
        let on = (0.0..1.0).contains(&across) && (0.0..1.0).contains(&down);
        Some((
            (
                measured(across.clamp(0.0, 1.0), self.shot.0),
                measured(down.clamp(0.0, 1.0), self.shot.1),
            ),
            on,
        ))
    }
}

#[derive(Default)]
pub(super) struct Driving {
    at: Option<(u32, u32)>,
    pressed: bool,
}

impl Widget for Live {
    type State = Driving;

    fn paint(&self, _state: &mut Driving, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let Some(screen) = self.screen(bounds) else {
            return;
        };

        ctx.draw_image(screen, self.feed.clone());
        ctx.draw_quad(
            Quad::new(screen)
                .radius(Corner::Item)
                .stroke(1.0, palette().outline),
        );
    }

    fn event(&self, state: &mut Driving, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();

        if let Event::PointerReleased {
            position, button, ..
        } = event
        {
            ctx.release_pointer();
            if !std::mem::take(&mut state.pressed) {
                return;
            }
            let Some(at) = self.edged(bounds, *position) else {
                return;
            };
            state.at = Some(at);
            ctx.set_handled();
            forward_input(
                ctx,
                MachineInput::PointerRelease {
                    button: *button,
                    at,
                },
            );
            return;
        }

        if !self.taking {
            return;
        }

        match event {
            Event::PointerMoved(at) => {
                let Some(at) = self.place(bounds, *at) else {
                    return;
                };
                if state.at.replace(at) == Some(at) {
                    return;
                }
                forward_input(ctx, MachineInput::PointerMove(at.0, at.1));
            }

            Event::PointerPressed {
                position,
                button,
                modifiers,
            } => {
                let Some(at) = self.place(bounds, *position) else {
                    return;
                };
                ctx.capture_pointer();
                ctx.request_focus();
                ctx.set_handled();
                state.at = Some(at);
                state.pressed = true;
                forward_input(
                    ctx,
                    MachineInput::PointerPress {
                        button: *button,
                        at,
                        modifiers: *modifiers,
                    },
                );
            }

            Event::Scroll {
                position, delta, ..
            } => {
                let Some(at) = self.place(bounds, *position) else {
                    return;
                };
                ctx.set_handled();
                forward_input(
                    ctx,
                    MachineInput::Scroll {
                        at,
                        across: f64::from(delta.offset().x / SCROLL_LINE),
                        down: f64::from(delta.offset().y / SCROLL_LINE),
                    },
                );
            }

            Event::KeyPressed { key, modifiers, .. } => {
                ctx.set_handled();
                forward_input(ctx, MachineInput::Key(Chord::new(*modifiers, *key)));
            }

            Event::Commit(text) => {
                ctx.set_handled();
                forward_input(ctx, MachineInput::Text(text.clone()));
            }

            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        self.taking.then_some(CursorShape::Crosshair)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE.when(self.taking)
    }
}

fn forward_input(ctx: &mut EventCtx<'_>, input: MachineInput) {
    ctx.run(act(move |project| project.forward_input(input.clone())));
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a fraction inside 0..1 over a length that is already a u32"
)]
fn measured(fraction: f32, length: u32) -> u32 {
    ((fraction * length as f32) as u32).min(length.saturating_sub(1))
}
