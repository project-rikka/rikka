use crate::context::InputCtx;

impl InputCtx<'_> {
    pub(crate) fn person_active(&mut self) {
        self.backend.interacted();
        self.wake_displays();
    }
}
