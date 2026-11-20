use rikka_compositor_desktop::{Target, WindowKind};
use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_shell::{ShellRole, shell_role};
use rikka_compositor_surface::SurfaceId;

use crate::context::WindowsCtx;

impl WindowsCtx<'_> {
    pub(crate) fn root_committed(&mut self, root: SurfaceId) {
        let Some(role) = shell_role(self.surfaces, root) else {
            return self.x11_surface_committed(root);
        };
        let Some(target) = self.windows.desktop.target_for_role(role) else {
            return;
        };
        match (target, role) {
            (Target::Window(window), ShellRole::Toplevel(toplevel)) => {
                let committed = self
                    .shell
                    .toplevel(toplevel)
                    .and_then(|state| state.window_geometry())
                    .or_else(|| self.surfaces.bounding_box(root));
                let Some(committed) = committed else {
                    return;
                };
                if let Some(record) = self.windows.desktop.window_mut(window) {
                    let at = record.geometry().loc;
                    let size = record.frame().outer_size(committed.size);
                    record.set_geometry(Rect::new(at, size));
                    record.set_inset(committed.loc);
                }
                self.stretched_committed(window);
            }
            (Target::Popup(popup), _) => self.popup_committed(popup),
            _ => {}
        }
    }

    fn x11_surface_committed(&mut self, root: SurfaceId) {
        let Some(window) = self
            .windows
            .desktop
            .window_for_surface(root)
            .filter(|window| {
                self.windows
                    .desktop
                    .window(*window)
                    .is_some_and(|record| matches!(record.kind(), WindowKind::X11(_)))
            })
        else {
            return;
        };
        let size = self.surfaces.get(root).and_then(|surface| surface.size());
        let Some(size) = size.filter(|size| size.w > 0 && size.h > 0) else {
            return;
        };
        if let Some(record) = self.windows.desktop.window_mut(window) {
            let at = record.geometry().loc;
            let size = record.frame().outer_size(Size::new(size.w, size.h));
            if record.geometry().size != size {
                record.set_geometry(Rect::new(at, size));
                record.set_inset(Point::zero());
            }
        }
        self.stretched_committed(window);
    }
}
