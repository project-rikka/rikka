use rikka_input::Chord;
use rikka_nested::{NestedView, NestedViewState, nested_view};
use rikka_types::{CursorShape, Point, PointerButton, Rect};
use rikka_ui::{Action, Behavior, Event, EventCtx, IntoHandler, Layout, PaintCtx, Style, Widget};

use crate::session::Session;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationButton {
    Back,
    Forward,
}

impl NavigationButton {
    fn from_button(button: PointerButton) -> Option<NavigationButton> {
        match button {
            PointerButton::Back => Some(NavigationButton::Back),
            PointerButton::Forward => Some(NavigationButton::Forward),
            _ => None,
        }
    }
}

#[must_use]
#[derive(Widget)]
pub struct WebSurface {
    view: NestedView,
    on_navigation_button: Action<(NavigationButton,)>,
    #[style]
    style: Style,
}

#[derive(Debug, Default)]
pub struct WebSurfaceState(NestedViewState);

pub fn web_view(session: &Session) -> WebSurface {
    WebSurface {
        view: nested_view(session.view()),
        on_navigation_button: Action::default(),
        style: Style::default(),
    }
}

impl WebSurface {
    pub fn reserve_shortcuts(mut self, chords: impl IntoIterator<Item = Chord>) -> WebSurface {
        self.view = self.view.reserve_shortcuts(chords);
        self
    }

    pub fn pass_through(
        mut self,
        yielded: impl Fn(&Event) -> bool + Send + Sync + 'static,
    ) -> WebSurface {
        self.view = self.view.pass_through(yielded);
        self
    }

    pub fn retain_focus(mut self, retained: bool) -> WebSurface {
        self.view = self.view.retain_focus(retained);
        self
    }

    pub fn on_navigation_button<Marker>(
        mut self,
        handler: impl IntoHandler<(NavigationButton,), (), Marker>,
    ) -> WebSurface {
        self.on_navigation_button = Action::new(handler);
        self
    }

    fn navigation(&self, event: &Event) -> Option<(NavigationButton, bool)> {
        if !self.on_navigation_button.is_set() {
            return None;
        }
        match event {
            Event::PointerPressed { button, .. } => {
                Some((NavigationButton::from_button(*button)?, true))
            }
            Event::PointerReleased { button, .. } => {
                Some((NavigationButton::from_button(*button)?, false))
            }
            _ => None,
        }
    }
}

impl Widget for WebSurface {
    type State = WebSurfaceState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn behavior(&self, state: &WebSurfaceState) -> Behavior {
        self.view.behavior(&state.0)
    }

    fn paint(&self, state: &mut WebSurfaceState, ctx: &mut PaintCtx<'_>) {
        self.view.paint(&mut state.0, ctx);
    }

    fn event(&self, state: &mut WebSurfaceState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Some((button, pressed)) = self.navigation(event) {
            ctx.set_handled();
            if pressed {
                ctx.act(&self.on_navigation_button, (button,));
            }
            return;
        }
        self.view.event(&mut state.0, event, ctx);
    }

    fn cursor(&self, state: &WebSurfaceState, bounds: Rect, point: Point) -> Option<CursorShape> {
        self.view.cursor(&state.0, bounds, point)
    }

    fn caret(&self, state: &WebSurfaceState) -> Option<Rect> {
        self.view.caret(&state.0)
    }
}
