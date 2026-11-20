use rikka_theme::Corner;
use rikka_types::{Color, CursorShape, Key, Point, PointerButton, Rect, RoundedRect, Size};
use rikka_ui::{
    Behavior, Curve, DurationToken, Easing, Element, Event, EventCtx, ImageSource, Layout, Length,
    PaintCtx, Quad, View, Widget, theme,
};
use shell_kit::{EXIT_DURATION, Placement};

use crate::aim::{Mode, Target};
use crate::selector::Selector;
use crate::shared::Shared;

const DIM: Color = Color::rgba(0.0, 0.0, 0.0, 0.42);

const FADE_IN: Curve = Curve::Tween {
    easing: Easing::Standard,
    duration: DurationToken::Short4,
};

const FADE_OUT: Curve = Curve::Tween {
    easing: Easing::StandardAccel,
    duration: EXIT_DURATION,
};

const SHOWN: &str = "shown";

const EDGE: f32 = 1.5;
const BOLD_EDGE: f32 = 3.0;
const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.45);
const HAIR: f32 = 1.0;

const MINIMUM: f32 = 4.0;

pub(super) fn marquee(place: Placement, shared: Shared, picture: Option<ImageSource>) -> Marquee {
    Marquee {
        place,
        shared,
        picture,
        leaving: false,
        overlay: None,
    }
}

pub(super) struct Marquee {
    place: Placement,
    shared: Shared,
    picture: Option<ImageSource>,
    leaving: bool,
    overlay: Option<Element>,
}

#[derive(Default)]
pub(crate) struct MarqueeState {
    pointer: Option<Point>,
    held: Option<Rect>,
}

struct Highlight {
    desktop: Rect,
    local: Rect,
    window: Option<String>,
}

impl Marquee {
    #[must_use]
    pub(crate) fn overlay(mut self, overlay: Option<Element>) -> Marquee {
        self.overlay = overlay;
        self
    }

    #[must_use]
    pub(crate) fn leaving(mut self, leaving: bool) -> Marquee {
        self.leaving = leaving;
        self
    }

    fn scanning(&self) -> bool {
        self.shared.aimed.mode() == Mode::Scan
    }

    fn output(&self) -> Rect {
        let size = match self.place.size.width >= 1.0 && self.place.size.height >= 1.0 {
            true => self.place.size,
            false => Size::new(1.0, 1.0),
        };
        Rect::from_origin_size(self.place.position, size)
    }

    fn highlight(&self, state: &MarqueeState) -> Option<Highlight> {
        let of = |desktop: Rect, window: Option<String>| Highlight {
            desktop,
            local: self.place.to_surface(desktop),
            window,
        };
        match self.shared.aimed.framing() {
            Target::Region => self.shared.selection.rect().map(|rect| of(rect, None)),
            Target::Screen => Some(of(self.output(), None)),
            Target::Window => {
                let at = self.place.to_desktop(state.pointer?);
                let window = self.shared.windows.at(at)?;
                Some(of(window.bounds, Some(window.id)))
            }
        }
    }

    fn code_at(&self, local: Point) -> Option<usize> {
        self.shared.codes.read().at(self.place.to_desktop(local))
    }

    fn shown_area(&self, state: &mut MarqueeState, bounds: Rect) -> Option<Rect> {
        if self.leaving {
            return state.held;
        }
        state.held = self
            .highlight(state)
            .map(|highlight| highlight.local)
            .filter(|local| {
                local
                    .intersection(&bounds)
                    .is_some_and(|shown| !shown.is_empty())
            });
        state.held
    }

    fn settle(&self, state: &MarqueeState, ctx: &mut EventCtx<'_>) {
        let highlight = self.highlight(state);
        if self.shared.aimed.framing() == Target::Region {
            match self.scanning() {
                true => self.shared.selection.clear(),
                false => self.shared.selection.settle(),
            }
        }
        let highlight = highlight.filter(|highlight| {
            highlight.desktop.width() >= MINIMUM && highlight.desktop.height() >= MINIMUM
        });
        match (highlight, self.scanning()) {
            (Some(highlight), _) => {
                let Highlight {
                    desktop, window, ..
                } = highlight;
                ctx.run(move |selector: &mut Selector| {
                    selector.picked(desktop, window.clone());
                });
            }
            (None, true) => {
                ctx.run(Selector::dismiss);
            }
            (None, false) => {
                ctx.run(Selector::cancelled);
            }
        }
    }

    fn shroud(ctx: &mut PaintCtx<'_>, bounds: Rect, area: Option<Rect>) {
        let quad = Quad::new(bounds).fill(DIM);
        let quad = match area.and_then(|area| area.intersection(&bounds)) {
            Some(hole) => quad.cut_out(hole),
            None => quad,
        };
        ctx.draw_quad(quad);
    }

    fn outline(ctx: &mut PaintCtx<'_>, rect: Rect, accent: Color) {
        ctx.draw_quad(Quad::new(rect.inflate(HAIR)).stroke(HAIR, SHADE));
        ctx.draw_quad(Quad::new(rect).stroke(EDGE, accent));
        ctx.draw_quad(Quad::new(rect.inflate(-EDGE)).stroke(HAIR, SHADE));
    }

    fn paint_codes(&self, state: &MarqueeState, ctx: &mut PaintCtx<'_>, accent: Color) {
        let bounds = ctx.bounds();
        let decoded = self.shared.codes.read();
        let hovered = state
            .pointer
            .and_then(|at| decoded.at(self.place.to_desktop(at)));
        for (index, code) in decoded.codes.iter().enumerate() {
            let local = self.place.to_surface(code.area);
            if !local.intersects(&bounds) {
                continue;
            }
            let shape = RoundedRect::new(local, Corner::Control);
            if let Some(picture) = &self.picture {
                ctx.clipped(shape, |ctx| ctx.draw_image(bounds, picture.clone()));
            }
            let marked = decoded.chosen == Some(index) || hovered == Some(index);
            let edge = if marked { BOLD_EDGE } else { EDGE };
            ctx.draw_quad(
                Quad::new(local.inflate(edge))
                    .stroke(HAIR, SHADE)
                    .radius(Corner::Control),
            );
            ctx.draw_quad(
                Quad::new(local)
                    .stroke(edge, accent)
                    .radius(Corner::Control),
            );
        }
        let pending = decoded
            .framing
            .map(|framing| framing.area)
            .or(decoded.missed);
        if let Some(area) = pending {
            Marquee::outline(ctx, self.place.to_surface(area), accent.with_alpha(0.6));
        }
    }

    fn scan_key(&self, key: Key, shift: bool, control: bool, ctx: &mut EventCtx<'_>) {
        let forward = match key {
            Key::Escape => {
                ctx.run(Selector::dismiss);
                None
            }
            Key::Enter => {
                ctx.run(Selector::primary);
                None
            }
            Key::Char('c' | 'C') if control => {
                ctx.run(Selector::copy);
                None
            }
            Key::Tab => Some(!shift),
            Key::ArrowRight | Key::ArrowDown => Some(true),
            Key::ArrowLeft | Key::ArrowUp => Some(false),
            _ => return,
        };
        if let Some(forward) = forward {
            ctx.run(move |selector: &mut Selector| selector.cycle(forward));
        }
        ctx.set_handled();
    }
}

impl Widget for Marquee {
    type State = MarqueeState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut MarqueeState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let shown = ctx.progress(SHOWN, !self.leaving, FADE_IN, FADE_OUT);
        let area = self.shown_area(state, bounds);
        let accent = theme().palette().accent;

        ctx.with_opacity(shown, |ctx| {
            if let Some(picture) = &self.picture {
                ctx.draw_image(bounds, picture.clone());
            }
            Marquee::shroud(ctx, bounds, area);
            if let Some(area) = area {
                Marquee::outline(ctx, area, accent);
            }
            if self.scanning() {
                self.paint_codes(state, ctx, accent);
            }
        });

        if self.shared.selection.is_dragging() {
            ctx.request_frame();
        }
    }

    fn event(&self, state: &mut MarqueeState, event: &Event, ctx: &mut EventCtx<'_>) {
        let dragging = self.shared.aimed.framing() == Target::Region;
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.pointer = Some(*position);
                ctx.set_handled();
                if self.scanning()
                    && let Some(index) = self.code_at(*position)
                {
                    ctx.run(move |selector: &mut Selector| selector.point(Some(index)));
                    return;
                }
                if !dragging {
                    return self.settle(state, ctx);
                }
                self.shared
                    .selection
                    .begin(self.place.to_desktop(*position));
                ctx.capture_pointer();
                ctx.request_paint();
            }
            Event::PointerPressed {
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.set_handled();
                ctx.run(Selector::cancelled);
            }
            Event::PointerMoved(position) => {
                state.pointer = Some(*position);
                if dragging {
                    self.shared
                        .selection
                        .extend(self.place.to_desktop(*position));
                }
                ctx.request_paint();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if dragging && self.shared.selection.is_dragging() => {
                ctx.release_pointer();
                ctx.set_handled();
                self.settle(state, ctx);
            }
            Event::PointerLeft => {
                state.pointer = None;
                ctx.request_paint();
            }
            Event::KeyPressed { key, modifiers, .. } if self.scanning() => {
                self.scan_key(*key, modifiers.shift, modifiers.control, ctx);
            }
            Event::KeyPressed { key, .. } => match key {
                Key::Escape => {
                    ctx.run(Selector::cancelled);
                }
                Key::Enter => self.settle(state, ctx),
                _ => {}
            },
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, point: Point) -> Option<CursorShape> {
        if self.scanning() && !self.shared.selection.is_dragging() && self.code_at(point).is_some()
        {
            return Some(CursorShape::Pointer);
        }
        Some(match self.shared.aimed.framing() {
            Target::Region => CursorShape::Crosshair,
            Target::Window | Target::Screen => CursorShape::Default,
        })
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS | Behavior::TAKES_TAB.when(self.scanning())
    }

    fn debug_name(&self) -> &'static str {
        "CaptureMarquee"
    }
}

impl std::fmt::Debug for Marquee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Marquee")
            .field("output", &self.place.name)
            .finish_non_exhaustive()
    }
}

impl View for Marquee {
    fn build(mut self) -> Element {
        match self.overlay.take() {
            Some(overlay) => Element::with_children(self, vec![overlay]),
            None => Element::new(self),
        }
    }
}
