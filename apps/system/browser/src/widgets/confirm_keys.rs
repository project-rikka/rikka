use rikka_app::prelude::*;
use rikka_ui::Handler;

pub(crate) fn confirm_keys<A, C>(
    content: impl View,
    accept: impl IntoHandler<(), (), A>,
    cancel: impl IntoHandler<(), (), C>,
) -> Element {
    Element::with_children(
        ConfirmKeys {
            accept: accept.into_handler(),
            cancel: cancel.into_handler(),
        },
        vec![content.build()],
    )
}

struct ConfirmKeys {
    accept: Handler,
    cancel: Handler,
}

impl Widget for ConfirmKeys {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::HUG, Length::HUG)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::FOCUSABLE | Behavior::TAKES_FOCUS | Behavior::NO_TAB_STOP
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed { .. } => {
                ctx.keep_focus();
                ctx.set_handled();
            }
            Event::KeyPressed { key, modifiers, .. } => {
                if modifiers.control || modifiers.alt || modifiers.logo {
                    return;
                }
                let answer = match key {
                    Key::Enter => &self.accept,
                    Key::Escape => &self.cancel,
                    _ => return,
                };
                ctx.call(answer, ());
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "ConfirmKeys"
    }
}
