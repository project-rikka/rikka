use rikka_theme::Corner;
use rikka_types::{Color, PointerButton, Rect};
use rikka_ui::{
    Align, Behavior, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx,
    Quad, View, Widget, palette, theme,
};

const RADIUS: f32 = Corner::Item.radius();

pub struct HighlightButton {
    child: Element,
    pressed: bool,
    wide: bool,
    radius: f32,
    background: Option<Color>,
    on_press: Handler<(Rect,)>,
}

pub fn highlight_button<Marker>(
    child: impl View,
    on_press: impl IntoHandler<(Rect,), (), Marker>,
) -> HighlightButton {
    HighlightButton {
        child: child.build(),
        pressed: false,
        wide: false,
        radius: RADIUS,
        background: None,
        on_press: on_press.into_handler(),
    }
}

impl HighlightButton {
    #[must_use]
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    #[must_use]
    pub fn wide(mut self) -> Self {
        self.wide = true;
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    #[must_use]
    pub fn on_background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }
}

impl Widget for HighlightButton {
    type State = ();

    fn layout(&self) -> Layout {
        let width = match self.wide {
            true => Length::FILL,
            false => Length::HUG,
        };
        Layout::row().align_items(Align::Center).width(width)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if !(ctx.hovered() || self.pressed) {
            return;
        }
        let background = self.background.unwrap_or(palette().background);
        ctx.draw_quad(
            Quad::new(ctx.bounds())
                .fill(theme().hovered(background))
                .radius(self.radius),
        );
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            button: PointerButton::Primary,
            position,
            ..
        } = event
        else {
            return;
        };
        let bounds = ctx.bounds();
        if !bounds.contains(*position) {
            return;
        }
        ctx.set_handled();
        ctx.call(&self.on_press, (bounds,));
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "HighlightButton"
    }
}

impl std::fmt::Debug for HighlightButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HighlightButton")
            .field("pressed", &self.pressed)
            .field("wide", &self.wide)
            .finish_non_exhaustive()
    }
}

impl View for HighlightButton {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
