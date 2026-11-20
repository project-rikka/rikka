use rikka_compositor_desktop::{Visibility, WindowId, Workspace};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_types::OutputId;
use rikka_session::workspaces::{COUNT, Displayed, State};

use super::Windows;
use crate::animation::slide_apart;
use crate::context::{WindowsCtx, WindowsRef, windows_ref};

impl Windows {
    fn holds(&self, workspace: Workspace) -> bool {
        self.desktop.windows().any(|(_, record)| {
            record.workspace() == Some(workspace)
                && matches!(
                    record.visibility(),
                    Visibility::Shown | Visibility::OnHiddenWorkspace
                )
        })
    }
}

impl WindowsCtx<'_> {
    fn send_one_to_workspace(&mut self, window: WindowId, number: usize) -> Option<OutputId> {
        let from = self
            .windows
            .desktop
            .window(window)
            .and_then(|record| record.workspace())?;
        let to = Workspace::new(from.output(), number);
        if from == to {
            return None;
        }
        if let Err(error) = self.windows.desktop.move_to_workspace(window, to) {
            tracing::debug!("cannot move window to workspace: {error}");
            return None;
        }
        self.forget_offered_snap(window);
        if super::around(self.windows, self.surfaces, from.output()).windows {
            self.animate_exit(window);
        }
        if self.windows.tiling.remove(window).is_some() {
            self.windows.place_tile(to, window, None);
        }
        Some(from.output())
    }

    pub(crate) fn show_workspace(&mut self, number: usize) {
        if let Some(output) = windows_ref!(self).active_output() {
            self.show_workspace_on(output, number);
        }
    }

    pub(crate) fn show_workspace_on(&mut self, output: OutputId, number: usize) {
        let switch = match self
            .windows
            .desktop
            .show_workspace(Workspace::new(output, number))
        {
            Ok(Some(switch)) => switch,
            Ok(None) => return,
            Err(error) => return tracing::debug!("cannot show workspace: {error}"),
        };
        tracing::debug!(
            "showing workspace {number}: {} window(s) hidden, {} shown",
            switch.leaving.len(),
            switch.arriving.len()
        );
        self.dismiss_snap_assist();
        let shows_windows = super::around(self.windows, self.surfaces, output).windows;
        let apart = match shows_windows {
            true => slide_apart(
                self.windows.full_area(output),
                number > switch.from.number(),
                switch
                    .leaving
                    .iter()
                    .filter_map(|window| windows_ref!(self).drawn_extent(*window)),
                switch
                    .arriving
                    .iter()
                    .filter_map(|window| windows_ref!(self).drawn_extent(*window)),
            ),
            false => 0.0,
        };
        let leaving = self.freeze_leaving(&switch.leaving);
        self.animations
            .switch(output, apart, leaving, switch.arriving);
        self.relayout(output);
    }

    pub(crate) fn send_to_workspace(&mut self, number: usize) {
        if let Some(window) = windows_ref!(self).focused_window() {
            self.send_window_to_workspace(window, number);
        }
    }

    pub(crate) fn send_window_to_workspace(&mut self, window: WindowId, number: usize) {
        let mut left = Vec::new();
        for member in windows_ref!(self).family_of(window) {
            if let Some(from) = self.send_one_to_workspace(member, number)
                && !left.contains(&from)
            {
                left.push(from);
            }
        }
        for output in left {
            self.relayout(output);
        }
    }
}

impl WindowsRef<'_> {
    pub(crate) fn displayed_workspaces(&self) -> Vec<Workspace> {
        self.windows
            .desktop
            .outputs()
            .filter(|(id, _)| self.outputs.wayland().is_published(*id))
            .filter_map(|(id, _)| self.windows.showing(id))
            .collect()
    }

    pub(crate) fn workspace_state(&self) -> State {
        State {
            count: COUNT,
            displays: self
                .windows
                .desktop
                .outputs()
                .filter_map(|(id, _)| {
                    let showing = self.windows.showing(id)?;
                    Some(Displayed {
                        output: self.outputs.wayland().name(id)?.to_owned(),
                        showing: showing.number(),
                        holding: std::array::from_fn(|slot| {
                            self.windows.holds(Workspace::new(id, slot + 1))
                        }),
                    })
                })
                .collect(),
        }
    }

    fn drawn_extent(&self, window: WindowId) -> Option<Rect<i32, Logical>> {
        let record = self.windows.desktop.window(window)?;
        let geometry = record.geometry();
        let Some(surfaces) = record
            .surface()
            .and_then(|root| self.surfaces.bounding_box(root))
        else {
            return Some(geometry);
        };
        let at = record.surface_location();
        Some(geometry.merge(Rect::new(surfaces.loc + at, surfaces.size)))
    }
}
