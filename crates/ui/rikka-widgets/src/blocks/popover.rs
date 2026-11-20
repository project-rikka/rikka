use rikka_types::{Key, PointerButton};
use rikka_ui::{
    Action, Behavior, Corner, Element, Event, EventCtx, Inset, IntoHandler, Layout, Length,
    PaintCtx, Quad, View, Widget, pop,
};

const PADDING: f32 = 12.0;

#[must_use]
pub struct Popover {
    content: Element,
    dismiss: Action,
    padding: f32,
}

pub fn popover(content: impl View) -> Popover {
    Popover {
        content: content.build(),
        dismiss: Action::default(),
        padding: PADDING,
    }
}

impl Popover {
    pub fn on_dismiss<Marker>(mut self, on_dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.dismiss = Action::new(on_dismiss);
        self
    }

    pub fn flush(mut self) -> Self {
        self.padding = 0.0;
        self
    }

    fn dismissed(&self, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        ctx.act(&self.dismiss, ());
    }
}

impl Widget for Popover {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
            .padding(Inset::all(self.padding))
            .size(Length::HUG, Length::HUG)
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::MODAL | Behavior::TRACKS_HOVER
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = ctx.theme().palette();
        ctx.draw_pane(
            Quad::new(ctx.bounds())
                .fill(palette.surface)
                .stroke(1.0, palette.outline)
                .radius(Corner::Panel),
        );
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                button, position, ..
            } if !ctx.bounds().contains(position) => {
                if button != PointerButton::Primary {
                    ctx.act(&self.dismiss, ());
                    return;
                }
                self.dismissed(ctx);
            }
            Event::PointerPressed { .. }
            | Event::PointerReleased { .. }
            | Event::Scroll { .. }
            | Event::Gesture { .. } => ctx.set_handled(),
            Event::KeyPressed {
                key: Key::Escape, ..
            } => self.dismissed(ctx),
            _ => {}
        }
    }
}

impl std::fmt::Debug for Popover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Popover").finish_non_exhaustive()
    }
}

impl View for Popover {
    fn build(mut self) -> Element {
        let content = std::mem::replace(&mut self.content, Element::empty());
        Element::with_children(self, vec![content]).transition(pop())
    }
}
