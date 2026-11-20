use rikka_compositor_desktop::{NewWindow, Placement, Tags, WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_shell::{ForeignToplevel, ToplevelConfigure, ToplevelId, ToplevelStates};
use rikka_compositor_types::OutputId;
use rikka_compositor_xwm::{WindowType, XWindowId, Xwm};

use super::Windows;
use super::arrange::is_tileable;
use super::decoration::{Decoration, client_configure, frame_extents};
use super::mode::{Enlarged, Mode, placement, states};
use super::tiling::Layout;
use super::{Enlargement, Enlargements};
use crate::context::{WindowsCtx, WindowsRef, windows_ref};

const CASCADE_START: i32 = 48;
const CASCADE_STEP: i32 = 32;
const CASCADE_STEPS: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Arrival {
    enlarged: Enlargements,
    plan: Option<Plan>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Plan {
    output: OutputId,
    at: Point<i32, Logical>,
    dialog: bool,
    configure: ToplevelConfigure,
}

pub(crate) fn cascade(area: Rect<i32, Logical>, windows: usize) -> Point<i32, Logical> {
    let step = CASCADE_STEP * (windows % CASCADE_STEPS) as i32;
    area.loc + Point::new(CASCADE_START + step, CASCADE_START + step)
}

fn foreign_toplevel(kind: WindowKind, xwm: Option<&Xwm>) -> Option<ForeignToplevel<'_>> {
    match kind {
        WindowKind::Xdg(toplevel) => Some(ForeignToplevel::Xdg(toplevel)),
        WindowKind::X11(x11) => xwm?.window(x11).map(|state| ForeignToplevel::Named {
            title: Some(&state.title),
            app_id: Some(&state.class),
        }),
    }
}

impl Windows {
    fn admit(&mut self, new: NewWindow, dialog: bool) -> Option<WindowId> {
        let window = match self.desktop.add_window(new) {
            Ok(window) => window,
            Err(error) => {
                tracing::warn!("cannot map window: {error}");
                return None;
            }
        };
        if let Some(record) = self.desktop.window_mut(window) {
            record.set_tags(if dialog { Tags::DIALOG } else { Tags::empty() });
        }
        Some(window)
    }
}

impl WindowsRef<'_> {
    pub(crate) fn active_output(&self) -> Option<OutputId> {
        self.windows.output_at(self.cursor.position())
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn plan_arrival(&mut self, toplevel: ToplevelId) {
        let carried = windows_ref!(self).dragged_along(toplevel);
        self.plan_arrival_carried(toplevel, carried);
    }

    pub(crate) fn follow_arrival(&mut self, toplevel: ToplevelId, at: Point<f64, Logical>) {
        let output = windows_ref!(self).active_output();
        let Some(plan) = self
            .windows
            .arrivals
            .get_mut(&toplevel)
            .and_then(|arrival| arrival.plan.as_mut())
        else {
            return;
        };
        plan.at = at.to_i32_round();
        if let Some(output) = output {
            plan.output = output;
        }
    }

    pub(crate) fn land_arrival(&mut self, toplevel: ToplevelId) {
        let tiles = self
            .windows
            .arrivals
            .get(&toplevel)
            .and_then(|arrival| arrival.plan)
            .is_some_and(|plan| !plan.dialog);
        if tiles && self.windows.tiling_mode() {
            self.plan_arrival_carried(toplevel, None);
        }
    }

    fn plan_arrival_carried(&mut self, toplevel: ToplevelId, dragged: Option<Point<f64, Logical>>) {
        let parent = windows_ref!(self).toplevel_parent(toplevel);
        let parent_output = parent.and_then(|parent| self.windows.output_of(parent));
        let Some(output) = parent_output.or_else(|| windows_ref!(self).active_output()) else {
            return;
        };
        let dialog = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|state| state.parent().is_some());
        let enlarged = self
            .windows
            .arrivals
            .get(&toplevel)
            .map_or_else(Enlargements::default, |arrival| arrival.enlarged);
        let area = self.windows.usable_area(output);
        let tiles = self.windows.tiling_mode() && !dialog && dragged.is_none();
        let tile = tiles.then(|| {
            let workspace = self.windows.showing(output);
            let next_to = windows_ref!(self).focused_window();
            let frame = self.windows.frame(output);
            match workspace.and_then(|workspace| self.windows.tiling.layout(workspace)) {
                Some(layout) => layout.arrival(frame, next_to),
                None => Layout::new(self.windows.tiling.kind(), frame).arrival(frame, None),
            }
        });
        let shaped = self
            .windows
            .shape_for(output, enlarged)
            .filter(|_| dragged.is_none());
        let at = match dragged {
            Some(at) => at.to_i32_round(),
            None => shaped.or(tile).map_or_else(
                || cascade(area, self.windows.desktop.windows().count()),
                |rect| rect.loc,
            ),
        };
        let placement = placement(enlarged, tile.is_some());
        let configure = ToplevelConfigure {
            size: shaped.or(tile).map(|rect| rect.size),
            states: states(placement, enlarged, false),
            bounds: Some(area.size),
            capabilities: None,
            decoration: None,
        };
        self.windows.arrivals.entry(toplevel).or_default().plan = Some(Plan {
            output,
            at,
            dialog,
            configure,
        });
        let decoration = self.decoration_of(WindowKind::Xdg(toplevel));
        let frame = decoration.frame(placement, true);
        self.shell
            .configure_toplevel(toplevel, client_configure(configure, frame, decoration));
    }

    pub(crate) fn arriving_enlarged(
        &mut self,
        toplevel: ToplevelId,
        mode: Enlargement,
        large: bool,
    ) {
        let entry = self.windows.arrivals.entry(toplevel).or_default();
        entry.enlarged = match large {
            true => entry.enlarged.with_mode(mode),
            false => entry.enlarged.without_mode(mode),
        };
        let sent = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|state| state.initial_configure_sent());
        if sent {
            self.plan_arrival(toplevel);
        }
    }

    pub(crate) fn decoration_chosen_early(&mut self, toplevel: ToplevelId) {
        let planned = self
            .windows
            .arrivals
            .get(&toplevel)
            .is_some_and(|arrival| arrival.plan.is_some());
        if planned {
            self.plan_arrival(toplevel);
        }
    }

    pub(crate) fn refresh_foreign(&mut self, window: WindowId) {
        let Some(record) = self.windows.desktop.window(window) else {
            return;
        };
        let Some(foreign) = self
            .windows
            .desktop
            .policy(window)
            .and_then(|policy| policy.foreign)
        else {
            return;
        };
        if let Some(info) = foreign_toplevel(record.kind(), self.x11.get()) {
            self.shell.update_published_toplevel(foreign, info);
        }
        self.windows.session_changed();
    }

    pub(crate) fn arrive_toplevel(&mut self, toplevel: ToplevelId) {
        let Some(state) = self.shell.toplevel(toplevel) else {
            return;
        };
        let surface = state.surface();
        let committed = state
            .window_geometry()
            .or_else(|| self.surfaces.bounding_box(surface));
        let size = committed.map_or_else(|| Size::new(1, 1), |rect| rect.size);
        let inset = committed.map_or_else(Point::zero, |rect| rect.loc);
        let Some(arrival) = self.windows.arrivals.remove(&toplevel) else {
            return;
        };
        let Some(plan) = arrival.plan else {
            return;
        };
        let parent = windows_ref!(self).toplevel_parent(toplevel);
        let workspace = parent
            .and_then(|parent| self.windows.desktop.window(parent)?.workspace())
            .or_else(|| self.windows.showing(plan.output));
        let outer = self
            .decoration_of(WindowKind::Xdg(toplevel))
            .frame(Placement::Floating, true)
            .outer_size(size);
        let at = match windows_ref!(self).dragged_along(toplevel) {
            Some(at) => at.to_i32_round(),
            None => parent
                .and_then(|parent| self.windows.dialog_spot(parent, outer))
                .unwrap_or(plan.at),
        };
        let mut new = NewWindow::xdg(toplevel, surface, Rect::new(at, size));
        if let Some(workspace) = workspace {
            new = new.with_workspace(workspace);
        }
        let Some(window) = self.windows.admit(new, plan.dialog) else {
            return;
        };
        let restore = Rect::new(
            cascade(
                self.windows.usable_area(plan.output),
                self.windows.desktop.windows().count(),
            ),
            outer,
        );
        if let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) {
            record.set_inset(inset);
            if arrival.enlarged.any() {
                policy.mode = Mode::Enlarged(Enlarged {
                    restore,
                    modes: arrival.enlarged,
                    snapped: None,
                });
            }
            record.set_placement(policy.mode.placement(false));
            policy.configure = plan.configure;
        }
        self.refresh_frame(window);
        self.welcome(window);
    }

    pub(crate) fn arrive_x11(&mut self, x11: XWindowId) {
        let Some(state) = self.x11.get().and_then(|xwm| xwm.window(x11)) else {
            return;
        };
        let unmanaged = state.override_redirect;
        let passive = state.window_type() == WindowType::Dnd;
        let dialog = !is_tileable(state.window_type(), state.transient_for.is_some());
        let transient_for = state.transient_for;
        let enlarged = Enlargements {
            maximized: state.is_maximized(),
            fullscreen: state.is_fullscreen(),
        };
        let size = Size::new(
            i32::try_from(state.geometry.width)
                .unwrap_or(i32::MAX)
                .max(1),
            i32::try_from(state.geometry.height)
                .unwrap_or(i32::MAX)
                .max(1),
        );
        let requested = Rect::new(Point::new(state.geometry.x, state.geometry.y), size);
        if unmanaged {
            let new = NewWindow::x11(x11, requested).unmanaged();
            match self.windows.desktop.add_window(new) {
                Ok(window) if passive => {
                    if let Some(record) = self.windows.desktop.window_mut(window) {
                        record.set_tags(Tags::PASSIVE);
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!("cannot map override-redirect X11 window: {error}");
                }
            }
            return self.windows.session_changed();
        }
        let frame = Decoration::of_x11(state).frame(Placement::Floating, true);
        let outer = frame.outer_size(size);
        self.x11.set_frame_extents(x11, frame_extents(frame));
        if !self.x11.map(x11) {
            return;
        }
        let parent = transient_for.and_then(|parent| self.windows.desktop.window_for_x11(parent));
        let parent_output = parent.and_then(|parent| self.windows.output_of(parent));
        let Some(output) = parent_output.or_else(|| windows_ref!(self).active_output()) else {
            return;
        };
        let area = self.windows.usable_area(output);
        let at = parent
            .and_then(|parent| self.windows.dialog_spot(parent, outer))
            .unwrap_or_else(|| cascade(area, self.windows.desktop.windows().count()));
        let workspace = parent
            .and_then(|parent| self.windows.desktop.window(parent)?.workspace())
            .or_else(|| self.windows.showing(output));
        let mut new = NewWindow::x11(x11, Rect::new(at, size));
        if let Some(workspace) = workspace {
            new = new.with_workspace(workspace);
        }
        let Some(window) = self.windows.admit(new, dialog) else {
            return;
        };
        if let Some(record) = self.windows.desktop.window_mut(window) {
            record.set_frame(frame);
        }
        match self.tile_window(window) {
            Some(output) => self.retile(output),
            None => {
                self.stand(window, Rect::new(at, outer));
            }
        }
        if enlarged.maximized {
            self.enlarge(window, Enlargement::Maximized, None);
        }
        if enlarged.fullscreen {
            self.enlarge(window, Enlargement::Fullscreen, None);
        }
        self.welcome(window);
    }

    fn welcome(&mut self, window: WindowId) {
        let foreign = self
            .windows
            .desktop
            .window(window)
            .and_then(|record| foreign_toplevel(record.kind(), self.x11.get()))
            .map(|info| self.shell.publish_toplevel(info));
        if let Some(foreign) = foreign
            && let Some(policy) = self.windows.desktop.policy_mut(window)
        {
            policy.foreign = Some(foreign);
        }
        if !self.windows.is_x11(window)
            && !windows_ref!(self).is_dragged(window)
            && let Some(output) = self.tile_window(window)
        {
            self.retile(output);
        }
        self.windows.queue_configure(window);
        self.animations.open(window);
        self.focus_window(Some(window));
        self.desktop_changed();
    }

    pub(crate) fn depart(&mut self, window: WindowId) {
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return;
        };
        let output = record.output();
        let foreign = policy.foreign.take();
        policy.configure.states.remove(ToplevelStates::ACTIVATED);
        if let Some(foreign) = foreign {
            self.shell.close_published_toplevel(foreign);
        }
        self.windows.captions.forget(window);
        self.windows
            .pending_configures
            .retain(|pending| *pending != window);
        self.windows.recency.forget(window);
        self.switch_lost(window);
        self.forget_tile(window);
        self.forget_offered_snap(window);
        if !self.exit_animated(window) {
            self.animations.forget(window);
            self.windows.desktop.remove_window(window);
        }
        self.refocus(output);
        self.sync_x11_stacking();
        self.desktop_changed();
    }

    pub(crate) fn departed_toplevel(&mut self, toplevel: ToplevelId) {
        self.windows.arrivals.remove(&toplevel);
        if let Some(window) = self.windows.desktop.window_for_toplevel(toplevel) {
            self.depart(window);
        }
    }
}
