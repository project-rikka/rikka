use rikka_app::prelude::*;
use rikka_ui::{Handler, Layout};

use super::hotkey;
use crate::chat::Chat;

pub(crate) fn talk_area(key: Option<Key>, body: impl View) -> Element {
    Element::with_children(
        TalkArea {
            key,
            talk: Handler::new(|chat: &mut Chat, pressed: bool| chat.push_to_talk(pressed)),
        },
        vec![body.build()],
    )
}

struct TalkArea {
    key: Option<Key>,
    talk: Handler<(bool,)>,
}

impl Widget for TalkArea {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Some(talk_key) = self.key else {
            return;
        };
        let pressed = match *event {
            Event::KeyPressed { key, .. } if hotkey::same(key, talk_key) => true,
            Event::KeyReleased { key, .. } if hotkey::same(key, talk_key) => false,
            _ => return,
        };
        ctx.call(&self.talk, (pressed,));
        ctx.set_handled();
    }

    fn debug_name(&self) -> &'static str {
        "TalkArea"
    }
}
