use rikka_app::prelude::*;
use rikka_ui::{Action, Element, Layout, motion};

pub(super) const CHIP: f32 = 40.0;
const GLYPH: f32 = 22.0;

pub(super) fn backdrop(content: impl View, dismiss: Action) -> Element {
    Element::with_children(Backdrop { dismiss }, vec![content.build()])
}

pub(super) fn hidden_while(hidden: bool, body: impl View) -> Element {
    Element::with_children(Hiding { hidden }, vec![body.build()])
        .opacity(match hidden {
            true => 0.0,
            false => 1.0,
        })
        .animate(Motion::Standard)
}

struct Hiding {
    hidden: bool,
}

impl Widget for Hiding {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::HUG)
            .align_items(Align::Center)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::INERT.when(self.hidden)
    }

    fn same(&self, other: &Self) -> bool {
        self.hidden == other.hidden
    }
}

struct Backdrop {
    dismiss: Action,
}

impl Widget for Backdrop {
    type State = bool;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, _pressed: &bool) -> Behavior {
        Behavior::MODAL
    }

    fn paint(&self, _pressed: &mut bool, ctx: &mut PaintCtx<'_>) {
        ctx.draw_scrim(ctx.bounds());
    }

    fn event(&self, pressed: &mut bool, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed { position, .. } | Event::PointerReleased { position, .. }
                if !*pressed && !ctx.bounds().contains(position) => {}
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } => {
                *pressed = true;
                ctx.keep_focus();
                ctx.set_handled();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                if std::mem::take(pressed) && ctx.bounds().contains(position) {
                    ctx.act(&self.dismiss, ());
                }
                ctx.set_handled();
            }
            Event::PointerPressed { .. }
            | Event::PointerReleased { .. }
            | Event::Scroll { .. }
            | Event::Gesture { .. } => {
                ctx.keep_focus();
                ctx.set_handled();
            }
            Event::KeyPressed {
                key: Key::Escape, ..
            } => {
                ctx.act(&self.dismiss, ());
                ctx.set_handled();
            }
            _ => {}
        }
    }
}

pub(super) fn chip(symbol: MaterialSymbol, press: Option<Handler<()>>) -> Element {
    let shade = theme().dim(palette().text, press.is_none());
    let glyph = icon(symbol).extent(Length::px(GLYPH)).color(shade).any();
    Element::with_children(Pressable { press, round: true }, vec![glyph])
}

pub(super) fn tapped(body: impl View, press: Handler<()>) -> Element {
    Element::with_children(
        Pressable {
            press: Some(press),
            round: false,
        },
        vec![body.build()],
    )
}

struct Pressable {
    press: Option<Handler<()>>,
    round: bool,
}

impl Widget for Pressable {
    type State = bool;

    fn layout(&self) -> Layout {
        let layout = Layout::column()
            .align_items(Align::Center)
            .justify(Justify::Center);
        match self.round {
            true => layout.size(Length::px(CHIP), Length::px(CHIP)),
            false => layout,
        }
    }

    fn behavior(&self, _pressed: &bool) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.round && self.press.is_some())
    }

    fn paint(&self, pressed: &mut bool, ctx: &mut PaintCtx<'_>) {
        if !self.round {
            return;
        }
        let target = match (
            self.press.is_some(),
            *pressed,
            ctx.hovered() || ctx.focused(),
        ) {
            (false, _, _) => 0.06,
            (true, true, _) => 0.28,
            (true, false, true) => 0.2,
            (true, false, false) => 0.1,
        };
        let alpha = ctx.animate_curve("chip", target, motion::HOVER);
        ctx.draw_quad(
            Quad::new(ctx.bounds())
                .fill(ctx.theme().palette().text.with_alpha(alpha))
                .radius(CHIP / 2.0),
        );
    }

    fn event(&self, pressed: &mut bool, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerEntered | Event::PointerExited => ctx.request_paint(),
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } => {
                *pressed = self.press.is_some();
                ctx.keep_focus();
                ctx.capture_pointer();
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.release_pointer();
                ctx.request_paint();
                ctx.set_handled();
                if std::mem::take(pressed)
                    && ctx.bounds().contains(position)
                    && let Some(press) = &self.press
                {
                    ctx.call(press, ());
                }
            }
            Event::PointerPressed { .. } | Event::PointerReleased { .. } => {
                ctx.keep_focus();
                ctx.set_handled();
            }
            Event::KeyPressed {
                key: Key::Enter | Key::Space,
                ..
            } if ctx.focused() => {
                if let Some(press) = &self.press {
                    ctx.call(press, ());
                }
                ctx.set_handled();
            }
            Event::FocusGained | Event::FocusLost => ctx.request_paint(),
            _ => {}
        }
    }

    fn cursor(&self, _pressed: &bool, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        self.press.as_ref().map(|_| CursorShape::Pointer)
    }
}
