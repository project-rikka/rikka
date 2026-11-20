use rikka_compositor_desktop::{Visibility, WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_session::toplevels::{Bounds, State, Toplevel};
use rikka_session::workspaces::Show;

use super::Windows;
use crate::context::{WindowsCtx, WindowsRef, windows_ref};
use crate::session::ToplevelRequest;
use crate::wayland::WorkspaceList;
use crate::windows::Enlargement;

impl WindowsCtx<'_> {
    pub(crate) fn announce_session(&mut self, workspace_list: &mut WorkspaceList) {
        if !std::mem::take(&mut self.windows.changed) {
            return;
        }
        let toplevels = self.toplevel_state();
        self.session.toplevels.publish(toplevels);
        self.publish_switch();
        if self.core.profile.is_full() {
            let workspaces = windows_ref!(self).workspace_state();
            self.session.workspaces.publish(workspaces);
            let displayed = windows_ref!(self).displayed_workspaces();
            workspace_list.publish(displayed, self.outputs.wayland());
        }
    }
}

fn bounds_of(geometry: Rect<i32, Logical>) -> Bounds {
    Bounds {
        x: geometry.loc.x,
        y: geometry.loc.y,
        width: geometry.size.w,
        height: geometry.size.h,
    }
}

impl Windows {
    pub(crate) fn session_changed(&mut self) {
        self.changed = true;
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn desktop_changed(&mut self) {
        self.frames.request_all();
        self.windows.session_changed();
    }

    fn toplevel_state(&mut self) -> State {
        let focused = windows_ref!(self).focused_window();
        let desktop = &self.windows.desktop;
        let shown = |id: &WindowId| {
            desktop
                .window(*id)
                .is_some_and(|record| record.visibility() == Visibility::Shown)
        };
        let ordered: Vec<WindowId> = desktop
            .stacking()
            .filter(shown)
            .chain(desktop.stacking().filter(|id| !shown(id)))
            .collect();
        State {
            toplevels: ordered
                .into_iter()
                .filter_map(|id| self.toplevel(id, focused))
                .collect(),
        }
    }

    pub(crate) fn toplevel(&mut self, id: WindowId, focused: Option<WindowId>) -> Option<Toplevel> {
        let (title, app_id) = self.names_of(id)?;
        let record = self.windows.desktop.window(id)?;
        let workspace = record.workspace()?;
        Some(Toplevel {
            id: self
                .shell
                .published_identifier(self.windows.desktop.policy(id)?.foreign?)?
                .to_owned(),
            app_id,
            title,
            output: self.outputs.wayland().name(workspace.output())?.to_owned(),
            workspace: workspace.number(),
            focused: focused == Some(id),
            bounds: bounds_of(record.geometry()),
            on_screen: record.visibility() == Visibility::Shown,
            minimized: record.visibility() == Visibility::Minimized,
            maximized: self.windows.enlargements(id).maximized,
            tiled: self.windows.is_tiled(id),
        })
    }

    fn names_of(&mut self, window: WindowId) -> Option<(String, String)> {
        if let Some(named) = self.x11_names(window) {
            return Some(named);
        }
        Some(match self.windows.desktop.window(window)?.kind() {
            WindowKind::Xdg(toplevel) => {
                self.shell
                    .toplevel(toplevel)
                    .map_or_else(Default::default, |state| {
                        (
                            state.title().unwrap_or_default().to_owned(),
                            state.app_id().unwrap_or_default().to_owned(),
                        )
                    })
            }
            WindowKind::X11(_) => Default::default(),
        })
    }

    fn x11_names(&mut self, window: WindowId) -> Option<(String, String)> {
        let WindowKind::X11(x11) = self.windows.desktop.window(window)?.kind() else {
            return None;
        };
        let state = self.x11.get()?.window(x11)?;
        Some((state.title.clone(), state.class.clone()))
    }

    pub(crate) fn apply_show(&mut self, Show { output, number }: Show) {
        match output {
            None => self.show_workspace(number),
            Some(name) => match self.outputs.wayland().named(&name) {
                Some(output) => self.show_workspace_on(output, number),
                None => tracing::warn!("no output named `{name}`"),
            },
        }
    }

    pub(crate) fn apply_toplevel_request(&mut self, request: ToplevelRequest, id: &str) {
        let Some(window) = windows_ref!(self).window_called(id) else {
            return tracing::debug!("no window with id `{id}`");
        };
        match request {
            ToplevelRequest::Focus => self.reveal_window(window),
            ToplevelRequest::Close => self.close_window(window),
            ToplevelRequest::Minimize => self.minimize_window(window),
            ToplevelRequest::Maximize => {
                self.reveal_window(window);
                self.enlarge(window, Enlargement::Maximized, None);
            }
            ToplevelRequest::Unmaximize => self.shrink(window, Enlargement::Maximized),
            ToplevelRequest::Move(number) => self.send_window_to_workspace(window, number),
        }
        self.windows.session_changed();
    }
}

impl WindowsRef<'_> {
    pub(crate) fn window_called(&self, id: &str) -> Option<WindowId> {
        self.windows
            .desktop
            .policies()
            .find(|(_, policy)| {
                policy
                    .foreign
                    .and_then(|foreign| self.shell.published_identifier(foreign))
                    == Some(id)
            })
            .map(|(id, _)| id)
    }
}
