use rikka_types::{Point, Rect};
use rikka_ui::{EventCtx, clipboard};

use crate::navigation::menu::MenuItem;
use crate::text::editing::Editing;
use crate::text::editing::state::EditingState;
use crate::text::menu::MenuOptions;

impl Editing {
    pub(crate) fn summon(&self, state: &mut EditingState, at: Point, ctx: &mut EventCtx<'_>) {
        if let Some((offset, affinity)) = self.cursor_at(state, at)
            && self.document.place_cursor_unless_selected(offset, affinity)
        {
            state.goal = None;
            self.changed(ctx);
        }
        let items = self.menu_items(ctx);
        state.menu.open(items, at, ctx);
    }

    pub(crate) fn open_from_keyboard(&self, state: &mut EditingState, ctx: &mut EventCtx<'_>) {
        let room = Rect::new(0.0, 0.0, ctx.bounds().width(), ctx.bounds().height());
        let caret = state.caret_within().filter(|caret| room.intersects(caret));
        let items = self.menu_items(ctx);
        state.menu.open_by_keys(items, caret, ctx);
    }

    fn menu_items(&self, ctx: &mut EventCtx<'_>) -> Vec<MenuItem> {
        let extras = ctx.answer(&self.context_items, ()).unwrap_or_default();
        MenuOptions {
            editable: self.options.editable,
            secret: self.options.secret,
            pasteable: self.pasteable(),
            extras,
        }
        .items(&self.document)
    }

    fn pasteable(&self) -> bool {
        clipboard::has_text() || (self.on_paste.is_set() && !clipboard::mime_types().is_empty())
    }
}
