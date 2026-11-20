use cut_project::Graphic;
use rikka_app::prelude::*;
use rikka_figure_editing::{Typing, TypingOutcome, paint_loop, paint_typing};

use super::items::{HIT_REACH, Item, text_hit, typeset_of};
use super::viewing::Viewing;
use super::{PointerState, Stage};
use crate::cut::Cut;

impl Stage {
    pub(super) fn type_event(
        &self,
        state: &mut PointerState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(draft) = self.typing() else {
            return false;
        };
        let Some(graphic) = self
            .item_of(draft.clip)
            .and_then(|item| item.visual.graphic().cloned())
            .filter(Graphic::is_text)
        else {
            return false;
        };
        let typeset = || typeset_of(&graphic).expect("the typed item is text");
        let typed = Typing::new(&draft.document, &mut state.menu)
            .caret(state.caret)
            .handle(event, typeset, ctx);
        match typed {
            TypingOutcome::Edited | TypingOutcome::Unchanged => true,
            TypingOutcome::Committed | TypingOutcome::Dropped => {
                ctx.run(Cut::stop_typing);
                true
            }
            TypingOutcome::Ignored => false,
        }
    }

    pub(super) fn summon_text_menu(
        &self,
        state: &mut PointerState,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(draft) = self.typing() else {
            return false;
        };
        let format = self.format();
        let viewing = Viewing::new(ctx.bounds(), format);
        let at = viewing.to_stage(position);
        let reach = HIT_REACH / viewing.scale.max(f32::EPSILON);
        let items = self.items();
        let Some(item) = items
            .iter()
            .find(|item| item.clip == draft.clip && item.hits(format, at, reach))
        else {
            return false;
        };
        let document = &draft.document;
        if let Some(hit) = text_hit(item, at) {
            document.place_cursor_unless_selected(hit.at, hit.affinity);
        }
        state.menu.open(text_menu(document), position, ctx);
        true
    }

    pub(super) fn paint_typing(
        &self,
        ctx: &mut PaintCtx<'_>,
        viewing: &Viewing,
        items: &[Item],
    ) -> Option<Rect> {
        let draft = self.typing()?;
        let item = items.iter().find(|item| item.clip == draft.clip)?;
        let typeset = typeset_of(item.visual.graphic()?)?;
        let projection = item.projection.as_ref()?.projection();
        let to_screen = |at: Point| projection.map(at).map(|spot| viewing.to_screen(spot));
        let [a, b, c, d] = typeset.layout().corners();
        paint_loop(
            ctx,
            &[to_screen(a)?, to_screen(b)?, to_screen(c)?, to_screen(d)?],
            palette().accent,
        );
        paint_typing(ctx, &typeset, &draft.document, &to_screen)
    }
}
