use crate::context::{InputCtx, switcher_ctx};

impl InputCtx<'_> {
    pub(crate) fn input_turn(&mut self) {
        switcher_ctx!(self).judge_idle_inhibitors();
        self.agent_turn();
        self.announce_caret();
    }
}
