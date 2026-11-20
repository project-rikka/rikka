use std::cell::{OnceCell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures_util::Stream;
use rikka_ime::{Panel, StatusArea};
use rikka_worker::Observable;

#[derive(Clone, Debug, Default)]
pub struct InputMethods {
    panel: Rc<OnceCell<Panel>>,
    shown: Rc<RefCell<Arc<StatusArea>>>,
    changed: Observable<()>,
}

impl InputMethods {
    pub(crate) fn hold(&self, panel: Panel) {
        if self.panel.set(panel).is_ok() {
            self.refresh_status_area();
        }
    }

    pub(super) fn refresh_status_area(&self) {
        let Some(panel) = self.panel.get() else {
            return;
        };
        let area = panel.status_area();
        if Arc::ptr_eq(&self.shown.replace(Arc::clone(&area)), &area) {
            return;
        }
        self.changed.set(());
    }

    #[must_use]
    pub fn status_area(&self) -> Arc<StatusArea> {
        Arc::clone(&self.shown.borrow())
    }

    pub fn trigger(&self, key: &str) {
        let Some(panel) = self.panel.get() else {
            return;
        };
        if let Err(error) = panel.trigger(key) {
            tracing::warn!("failed to forward `{key}` to the input method: {error}");
        }
    }

    pub fn fold(&self) {
        if let Some(panel) = self.panel.get() {
            panel.close_menu();
        }
    }

    pub fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.changed.changes()
    }
}
