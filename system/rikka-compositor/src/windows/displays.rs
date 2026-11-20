use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::{OutputId, OutputLayout};

use crate::context::WindowsCtx;

impl WindowsCtx<'_> {
    fn display_changed(&mut self, output: OutputId, previous: OutputLayout, layout: OutputLayout) {
        let delta = layout.geometry.loc - previous.geometry.loc;
        if delta != Point::zero() {
            self.shift_windows(output, delta);
        }
        self.arrange_layers(output);
        self.work_area_changed(output);
        if previous.geometry.size != layout.geometry.size
            && let Some((lock, _)) = self.windows.desktop.lock_surface_on(output)
        {
            self.shell
                .configure_lock_surface(lock, layout.geometry.size);
        }
    }

    fn shift_windows(&mut self, output: OutputId, delta: Point<i32, Logical>) {
        let carried: Vec<WindowId> = self
            .windows
            .desktop
            .windows()
            .filter(|(id, record)| {
                record.output() == Some(output)
                    && !self.windows.is_tiled(*id)
                    && self
                        .windows
                        .desktop
                        .policy(*id)
                        .is_some_and(|policy| policy.mode.enlarged().is_none())
            })
            .map(|(id, _)| id)
            .collect();
        for window in carried {
            let Some(at) = self
                .windows
                .desktop
                .window(window)
                .map(|record| record.geometry().loc)
            else {
                continue;
            };
            self.move_window(window, at + delta);
        }
    }

    pub(crate) fn windows_follow_displays(
        &mut self,
        followed: &[(OutputId, OutputLayout)],
        shown: &[OutputId],
    ) {
        let layouts = self.outputs.wayland().layouts();
        let previous = |output: OutputId| {
            followed
                .iter()
                .find(|(followed, _)| *followed == output)
                .map(|(_, layout)| *layout)
        };
        let gone: Vec<OutputId> = followed
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !shown.contains(id))
            .collect();
        let moved = !gone.is_empty()
            || shown
                .iter()
                .any(|output| previous(*output) != layouts.get(*output));
        if !moved {
            return;
        }
        let anchors = self.windows.output_anchors();
        for output in gone {
            self.display_gone(output);
        }
        for output in shown {
            match (previous(*output), layouts.get(*output)) {
                (Some(before), Some(now)) if before == now => {}
                (Some(before), Some(now)) => {
                    self.display_changed(*output, before, now);
                }
                (None, Some(_)) => self.display_arrived(*output),
                (_, None) => {}
            }
        }
        self.restore_anchors(anchors);
    }

    fn display_arrived(&mut self, output: OutputId) {
        match self.windows.desktop.add_output(output) {
            Ok(strays) if !strays.is_empty() => {
                tracing::debug!("{} window(s) moved back to an output", strays.len());
                for window in strays {
                    self.tile_window(window);
                }
            }
            Ok(_) => {}
            Err(error) => return tracing::warn!("cannot add output to desktop: {error}"),
        }
        self.arrange_layers(output);
        self.retile(output);
        self.refocus(None);
    }

    fn display_gone(&mut self, output: OutputId) {
        let orphans = self.windows.tiling.forget(output);
        let removed = match self.windows.desktop.remove_output(output) {
            Ok(removed) => removed,
            Err(error) => return tracing::warn!("cannot remove output: {error}"),
        };
        for layer in removed.layers {
            self.shell.close_layer(layer);
            self.windows.forget_layer(layer);
        }
        for (number, window) in orphans {
            let Some(workspace) = self
                .windows
                .desktop
                .window(window)
                .and_then(|record| record.workspace())
            else {
                continue;
            };
            let onto = rikka_compositor_desktop::Workspace::new(workspace.output(), number);
            self.windows.place_tile(onto, window, None);
        }
        let refuge = self.windows.desktop.outputs().next().map(|(id, _)| id);
        if let Some(refuge) = refuge {
            self.retile(refuge);
        }
        self.refocus(None);
    }
}
