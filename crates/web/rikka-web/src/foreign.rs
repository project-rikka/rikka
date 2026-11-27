use std::sync::Weak;

use rikka_nested::View;

use crate::ids::WindowId;
use crate::runtime::Core;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForeignKind {
    DevTools,
}

pub struct ForeignWindow {
    pub kind: ForeignKind,
    pub(crate) view: View,
    pub(crate) window: WindowId,
    pub(crate) core: Weak<Core>,
}

impl ForeignWindow {
    pub(crate) fn view(&self) -> &View {
        &self.view
    }

    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.view.look().title.clone()
    }

    pub fn close(&self) {
        if let Some(core) = self.core.upgrade() {
            core.close_window(self.window);
        }
    }

    pub fn on_closed(&self, closed: impl FnOnce() + Send + 'static) {
        let Some(core) = self.core.upgrade() else {
            closed();
            return;
        };
        let mut state = core.state();
        match state.foreign.get_mut(&self.window) {
            Some(watchers) => watchers.push(Box::new(closed)),
            None => {
                drop(state);
                closed();
            }
        }
    }
}

impl std::fmt::Debug for ForeignWindow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ForeignWindow")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
