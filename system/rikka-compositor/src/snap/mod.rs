mod assist;
mod element;
mod motion;
mod offer;
mod paint;
mod staged;
mod state;

pub(crate) use rikka_compositor_snap::Divider;
pub(crate) use state::Snapping;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_snap::{between, pulled, shift};
use rikka_compositor_tiling::{EDGE_REACH, Edges};
use rikka_compositor_types::OutputId;

use crate::windows::{Snapped, gripped};

use crate::context::{WindowsCtx, WindowsRef, windows_ref};
use crate::windows::Windows;
use state::{Target, Where};

const SMALLEST: i32 = 160;

#[derive(Debug)]
pub(crate) struct Group {
    pub(crate) windows: Vec<WindowId>,
    pub(crate) rects: Vec<Rect<i32, Logical>>,
    pub(crate) minima: Vec<Size<i32, Logical>>,
}

impl Group {
    pub(crate) fn moved(
        &self,
        divider: Divider,
        delta: i32,
    ) -> Vec<(WindowId, Rect<i32, Logical>)> {
        shift(&self.rects, &self.minima, divider, delta)
            .into_iter()
            .zip(&self.windows)
            .filter_map(|(rect, window)| Some((*window, rect?)))
            .collect()
    }
}

fn rescaled(
    zone: Rect<i32, Logical>,
    from: Rect<i32, Logical>,
    to: Rect<i32, Logical>,
) -> Rect<i32, Logical> {
    if from == to {
        return zone;
    }
    if from.size.w <= 0 || from.size.h <= 0 {
        return to;
    }
    let along =
        |near: i32, span: i32, was_near: i32, was_span: i32, now_near: i32, now_span: i32| {
            let share = |value: i32| f64::from(value - was_near) / f64::from(was_span);
            let placed = |share: f64| now_near + (share * f64::from(now_span)).round() as i32;
            let start = placed(share(near));
            (start, placed(share(near + span)) - start)
        };
    let (x, w) = along(
        zone.loc.x,
        zone.size.w,
        from.loc.x,
        from.size.w,
        to.loc.x,
        to.size.w,
    );
    let (y, h) = along(
        zone.loc.y,
        zone.size.h,
        from.loc.y,
        from.size.h,
        to.loc.y,
        to.size.h,
    );
    Rect::new(Point::new(x, y), Size::new(w, h))
}

impl Windows {
    fn snaps_windows(&self) -> bool {
        let rules = self.rules();
        !self.tiling_mode() && rules.snap.windows && !self.desktop.locked()
    }

    pub(crate) fn snapped(&self, window: WindowId) -> Option<Snapped> {
        self.desktop.policy(window)?.mode.snapped()
    }

    pub(crate) fn snapped_zone_moved(&mut self, window: WindowId, zone: Rect<i32, Logical>) {
        if let Some(policy) = self.desktop.policy_mut(window)
            && let Some(snapped) = policy.mode.snapped_mut()
        {
            snapped.zone = zone;
        }
    }

    fn snappable(&self, window: WindowId) -> bool {
        self.snaps_windows()
            && self
                .desktop
                .window(window)
                .is_some_and(|record| !record.unmanaged())
            && !self.enlargements(window).fullscreen
            && !self.is_tiled(window)
    }

    pub(crate) fn forget_snap(&mut self, window: WindowId) {
        let unsnapped = self
            .desktop
            .policy_mut(window)
            .and_then(|policy| policy.mode.unsnap());
        if unsnapped.is_some() {
            self.session_changed();
        }
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn snap_drag_started(&mut self, window: WindowId, at: Point<f64, Logical>) {
        self.windows.forget_snap(window);
        if !self.windows.snappable(window) {
            return;
        }
        let Some(place) = windows_ref!(self).place_at(at) else {
            return;
        };
        self.dismiss_snap_assist();
        self.snap.begin(window, place);
    }

    pub(crate) fn snap_dragged(&mut self, at: Point<f64, Logical>) {
        let Some(place) = windows_ref!(self).place_at(at) else {
            return;
        };
        let rules = self.windows.rules().snap;
        let output = place.output;
        if self.snap.dragged(place, at, rules) {
            self.redraw(output);
        }
    }

    pub(crate) fn unsnap_under_pointer(
        &mut self,
        window: WindowId,
        pointer: Point<f64, Logical>,
    ) -> Option<Point<i32, Logical>> {
        let (record, policy) = self.windows.desktop.window_and_policy_mut(window)?;
        let snapped = policy.mode.unsnap()?;
        let from = record.geometry();
        let to = Rect::new(
            gripped(from, snapped.restore.size, pointer),
            snapped.restore.size,
        );
        if !self.stand(window, to) {
            return None;
        }
        self.animations.resize(window, from.size, to.size);
        self.windows.session_changed();
        Some(to.loc)
    }

    pub(crate) fn forget_every_snap(&mut self) {
        let windows: Vec<WindowId> = self.windows.desktop.windows().map(|(id, _)| id).collect();
        for window in windows {
            if let Some(policy) = self.windows.desktop.policy_mut(window) {
                policy.mode.unsnap();
            }
        }
        self.snap.forget_assist();
    }

    pub(crate) fn refit_snapped(&mut self, output: OutputId) {
        let area = self.windows.usable_area(output);
        let refitted: Vec<(WindowId, Snapped)> = self
            .windows
            .desktop
            .policies()
            .filter(|(id, _)| self.windows.output_of(*id) == Some(output))
            .filter(|(id, _)| !self.windows.enlargements(*id).any())
            .filter_map(|(id, policy)| Some((id, policy.mode.snapped()?)))
            .filter(|(_, snapped)| snapped.area != area)
            .collect();
        for (window, previous) in refitted {
            let zone = rescaled(previous.zone, previous.area, area);
            if let Some(snapped) = self
                .windows
                .desktop
                .policy_mut(window)
                .and_then(|policy| policy.mode.snapped_mut())
            {
                *snapped = Snapped {
                    zone,
                    area,
                    ..previous
                };
            }
            self.stand(window, zone);
        }
    }

    pub(crate) fn snap_divider_at(&mut self, at: Point<f64, Logical>) -> Option<(Group, Divider)> {
        if !self.windows.snaps_windows() || !self.windows.rules().snap.resize_together {
            return None;
        }
        let output = self.windows.desktop.output_at(at)?;
        let group = self.snap_group(output);
        let divider = between(&group.rects, at, EDGE_REACH)?;
        Some((group, divider))
    }

    pub(crate) fn snap_divider_pulled(
        &mut self,
        window: WindowId,
        edges: Edges,
    ) -> Option<(Group, Divider)> {
        if !self.windows.snaps_windows()
            || !self.windows.rules().snap.resize_together
            || self.windows.snapped(window).is_none()
        {
            return None;
        }
        let output = self.windows.output_of(window)?;
        let group = self.snap_group(output);
        let index = group.windows.iter().position(|member| *member == window)?;
        let divider = pulled(&group.rects, index, edges)?;
        Some((group, divider))
    }

    fn snap_group(&mut self, output: OutputId) -> Group {
        let standing: Vec<(WindowId, Rect<i32, Logical>)> = self
            .windows
            .desktop
            .stacking()
            .filter(|window| self.windows.output_of(*window) == Some(output))
            .filter(|window| {
                self.windows
                    .desktop
                    .window(*window)
                    .is_some_and(|record| record.visibility().shown())
            })
            .filter(|window| {
                self.windows.snapped(*window).is_some() && !self.windows.enlargements(*window).any()
            })
            .filter_map(|window| Some((window, self.windows.desktop.window(window)?.geometry())))
            .collect();
        let minima = standing
            .iter()
            .map(|(window, _)| self.smallest(*window))
            .collect();
        let (windows, rects) = standing.into_iter().unzip();
        Group {
            windows,
            rects,
            minima,
        }
    }

    pub(crate) fn divide(&mut self, moved: Vec<(WindowId, Rect<i32, Logical>)>) {
        for (window, rect) in moved {
            if self.stand(window, rect) {
                self.stretch_to(window, rect, None);
                self.windows.snapped_zone_moved(window, rect);
            }
        }
    }

    fn smallest(&mut self, window: WindowId) -> Size<i32, Logical> {
        let (least, _) = self.size_limits(window);
        Size::new(least.w.max(SMALLEST), least.h.max(SMALLEST))
    }

    pub(crate) fn snap_dropped(&mut self) -> bool {
        let Some(drag) = self.snap.drag() else {
            return false;
        };
        let (window, output, area) = (drag.window(), drag.output(), drag.area());
        let (target, layout) = (drag.target(), drag.layout());
        self.snap.end();
        self.redraw(output);

        let Some(Target { rect, maximize }) = target else {
            return false;
        };
        if maximize {
            self.windows.forget_snap(window);
            self.enlarge(window, crate::windows::Enlargement::Maximized, None);
            return true;
        }
        if !self.snap_window_to(window, rect, area) {
            return false;
        }
        if let Some(layout) = layout {
            self.offer_snap_assist(output, area, layout);
        }
        true
    }

    fn snap_window_to(
        &mut self,
        window: WindowId,
        zone: Rect<i32, Logical>,
        area: Rect<i32, Logical>,
    ) -> bool {
        if !self.windows.snappable(window) {
            return false;
        }
        let Some(record) = self.windows.desktop.window(window) else {
            return false;
        };
        let from = record.geometry();
        let snapped = self.windows.mode_of(window).snapped_into(from, zone, area);
        self.windows.set_mode(window, snapped);
        self.sync_x11_maximized(window, crate::windows::Enlargements::default());
        self.travel(window, zone);
        self.focus_window(Some(window));
        self.windows.session_changed();
        true
    }
}

impl WindowsRef<'_> {
    fn place_at(&self, at: Point<f64, Logical>) -> Option<Where> {
        let output = self
            .windows
            .desktop
            .output_at(at)
            .or_else(|| self.active_output())?;
        Some(Where {
            output,
            screen: self.windows.full_area(output),
            area: self.windows.usable_area(output),
        })
    }
}
