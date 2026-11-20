use rikka_app::prelude::*;
use rikka_ui::Handler;

pub(crate) fn floating<M>(content: impl View, dismiss: impl IntoHandler<(), (), M>) -> Element {
    Element::with_children(
        Floating {
            dismiss: dismiss.into_handler(),
        },
        vec![content.build()],
    )
}

struct Floating {
    dismiss: Handler,
}

impl Widget for Floating {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::HUG, Length::HUG)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::MODAL
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed { position, .. } if !ctx.bounds().contains(position) => {
                ctx.set_handled();
                ctx.call(&self.dismiss, ());
            }
            Event::PointerPressed { .. } | Event::PointerReleased { .. } => ctx.set_handled(),
            Event::KeyPressed {
                key: Key::Escape, ..
            } => {
                ctx.set_handled();
                ctx.call(&self.dismiss, ());
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "Floating"
    }
}
