use rikka_compositor_shell::{PopupEvent, PopupId};

use crate::context::WindowsCtx;
use crate::context::switcher_ctx;

impl WindowsCtx<'_> {
    fn adopt_popup(&mut self, popup: PopupId) {
        if self.windows.desktop.popup(popup).is_some() {
            return;
        }
        let Some(state) = self.shell.popup(popup) else {
            return;
        };
        let Some(parent) = state.parent() else {
            return;
        };
        let surface = state.surface();
        if let Err(error) = self.windows.desktop.add_popup(popup, parent, surface) {
            tracing::debug!("dismissing popup without a parent: {error}");
            self.shell.dismiss_popup(popup);
        }
    }

    fn place_popup(&mut self, popup: PopupId) {
        let Some(positioner) = self.shell.popup(popup).map(|state| *state.positioner()) else {
            return;
        };
        let geometry = match self.windows.desktop.constrain_popup(popup, &positioner) {
            Ok(geometry) => geometry,
            Err(error) => return tracing::debug!("cannot place popup: {error}"),
        };
        if let Some(record) = self.windows.desktop.popup_mut(popup) {
            record.set_geometry(geometry);
        }
        self.shell.configure_popup(popup, geometry);
    }

    pub(crate) fn popup_committed(&mut self, popup: PopupId) {
        let inset = self
            .shell
            .popup(popup)
            .and_then(|state| state.window_geometry())
            .map(|geometry| geometry.loc);
        if let (Some(inset), Some(record)) = (inset, self.windows.desktop.popup_mut(popup)) {
            record.set_inset(inset);
        }
    }

    pub(crate) fn popup_event(&mut self, popup: PopupId, event: PopupEvent) {
        match event {
            PopupEvent::Created => self.adopt_popup(popup),
            PopupEvent::InitialCommit | PopupEvent::Reposition { .. } => {
                self.adopt_popup(popup);
                self.place_popup(popup);
            }
            PopupEvent::Mapped => {
                self.popup_committed(popup);
                self.frames.request_all();
            }
            PopupEvent::Configured { .. } => self.popup_committed(popup),
            PopupEvent::Unmapped | PopupEvent::Destroyed => self.popup_gone(popup),
            PopupEvent::Grab { seat, serial } => {
                switcher_ctx!(self).grab_popup(popup, &seat, serial)
            }
        }
    }

    fn popup_gone(&mut self, popup: PopupId) {
        self.windows.desktop.remove_popup(popup);
        self.frames.request_all();
        self.popup_released(popup);
    }
}
