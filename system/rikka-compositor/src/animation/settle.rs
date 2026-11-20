use super::animations::Finished;
use crate::context::WindowsCtx;

impl WindowsCtx<'_> {
    pub(crate) fn finish_animations(&mut self) {
        let done = self.animations.take_done();
        if done.is_empty() {
            return;
        }
        for done in done {
            match done {
                Finished::Closed { window, snapshot } => {
                    self.windows.desktop.remove_window(window);
                    self.scenes.thaw(snapshot);
                }
                Finished::Left(snapshot) => self.scenes.thaw(snapshot),
            }
        }
        self.frames.request_all();
    }

    pub(crate) fn follow_animation_policy(&mut self) {
        self.animations.enable(self.policy.animates());
        self.finish_animations();
    }
}
