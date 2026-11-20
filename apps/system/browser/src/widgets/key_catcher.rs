use rikka_app::prelude::*;
use rikka_ui::Handler;

pub(crate) fn key_catcher<M>(
    content: impl View,
    on_key: impl IntoHandler<(Key, Modifiers), bool, M>,
) -> Element {
    Element::with_children(
        KeyCatcher {
            on_key: on_key.into_handler(),
        },
        vec![content.build()],
    )
}

struct KeyCatcher {
    on_key: Handler<(Key, Modifiers), bool>,
}

impl Widget for KeyCatcher {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::KeyPressed { key, modifiers, .. } = *event else {
            return;
        };
        if ctx.call(&self.on_key, (key, modifiers)) == Some(true) {
            ctx.set_handled();
        }
    }

    fn debug_name(&self) -> &'static str {
        "KeyCatcher"
    }
}
