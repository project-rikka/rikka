use rikka_app::prelude::*;
use rikka_ui::Action;
use rikka_web::{WebSurface, WebSurfaceState};

use crate::shell::Shell;
use browser_model::TabId;

pub(crate) fn pane_focus(content: impl View, tab: TabId) -> Element {
    Element::with_children(
        PaneFocus {
            tab,
            focus: Action::new(move |shell: &mut Shell| shell.focus_pane(tab)),
        },
        vec![content.build()],
    )
}

struct PaneFocus {
    tab: TabId,
    focus: Action,
}

impl Widget for PaneFocus {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.act(&self.focus, ());
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.tab == other.tab && self.focus == other.focus
    }

    fn debug_name(&self) -> &'static str {
        "PaneFocus"
    }
}

pub(crate) fn focusing(surface: WebSurface, tab: TabId) -> Element {
    Element::new(Focusing {
        surface,
        focus: Action::new(move |shell: &mut Shell| shell.focus_pane(tab)),
    })
}

struct Focusing {
    surface: WebSurface,
    focus: Action,
}

impl Widget for Focusing {
    type State = WebSurfaceState;

    fn layout(&self) -> Layout {
        self.surface.layout()
    }

    fn behavior(&self, state: &WebSurfaceState) -> Behavior {
        self.surface.behavior(state)
    }

    fn paint(&self, state: &mut WebSurfaceState, ctx: &mut PaintCtx<'_>) {
        self.surface.paint(state, ctx);
    }

    fn event(&self, state: &mut WebSurfaceState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.act(&self.focus, ());
        }
        self.surface.event(state, event, ctx);
    }

    fn cursor(&self, state: &WebSurfaceState, bounds: Rect, point: Point) -> Option<CursorShape> {
        self.surface.cursor(state, bounds, point)
    }

    fn caret(&self, state: &WebSurfaceState) -> Option<Rect> {
        self.surface.caret(state)
    }

    fn debug_name(&self) -> &'static str {
        "WebSurface"
    }
}
