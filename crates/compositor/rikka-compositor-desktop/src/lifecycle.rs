use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::ids::{SnapshotId, WindowId};
use crate::visibility::Visibility;

impl<P> Desktop<P> {
    pub fn minimize(&mut self, id: WindowId) -> Result<bool> {
        let window = self.known_mut(id)?;
        if window.unmanaged {
            return Err(Error::Unmanaged(id));
        }
        match window.visibility {
            Visibility::Exiting(_) => Err(Error::Exiting(id)),
            Visibility::Minimized => Ok(false),
            Visibility::Shown | Visibility::OnHiddenWorkspace => {
                window.visibility = Visibility::Minimized;
                Ok(true)
            }
        }
    }

    pub fn unminimize(&mut self, id: WindowId) -> Result<bool> {
        let window = self.known_mut(id)?;
        if window.visibility != Visibility::Minimized {
            return Ok(false);
        }
        window.visibility = Visibility::Shown;
        self.settle(id);
        self.raise(id)?;
        Ok(true)
    }

    pub fn exit_window(&mut self, id: WindowId, snapshot: SnapshotId) -> Result<bool> {
        let window = self.known(id)?;
        if !self.on_screen(window) || !window.visibility.shown() {
            self.remove_window(id);
            return Ok(false);
        }
        let kind = window.kind;
        self.forget_kind(id, kind);
        if let Some(popups) = self.window_popups.get_mut(id).map(std::mem::take) {
            self.drop_popups(popups);
        }
        let window = self.known_mut(id)?;
        let surface = window.surface.take();
        window.visibility = Visibility::Exiting(snapshot);
        self.forget_root(id, surface);
        Ok(true)
    }
}
