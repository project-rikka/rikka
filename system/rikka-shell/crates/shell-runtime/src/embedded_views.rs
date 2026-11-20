use rikka_types::WindowView;
use rikka_window_view::Shown;

#[derive(Debug)]
pub(super) struct EmbeddedViews {
    shown: Shown,
    built: Vec<WindowView>,
    sealed: Vec<WindowView>,
    rebuilt: bool,
    resealed: bool,
}

impl EmbeddedViews {
    pub(crate) fn new(shown: Shown) -> EmbeddedViews {
        EmbeddedViews {
            shown,
            built: Vec::new(),
            sealed: Vec::new(),
            rebuilt: false,
            resealed: false,
        }
    }

    pub(super) fn set_windows(&mut self, windows: &[WindowView]) {
        if self.built != windows {
            self.built.clear();
            self.built.extend_from_slice(windows);
            self.rebuilt = true;
        }
    }

    pub(super) fn commit(&mut self) {
        if std::mem::take(&mut self.rebuilt) {
            self.sealed.clone_from(&self.built);
            self.resealed = true;
        }
    }

    pub(crate) fn publish(&mut self) {
        if std::mem::take(&mut self.resealed) {
            self.shown.show(&self.sealed);
        }
    }
}
