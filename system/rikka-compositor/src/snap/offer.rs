use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_snap::{Layout, shape_of, slots};
use rikka_compositor_types::OutputId;
use rikka_types::Key;

use super::assist::{Assist, Card};
use crate::context::WindowsCtx;
use crate::snap::Snapping;
use crate::windows::Windows;

const TAKEN: i64 = 70;

fn covers(geometry: Rect<i32, Logical>, zone: Rect<i32, Logical>) -> bool {
    let Some(shared) = geometry.intersection(zone) else {
        return false;
    };
    let whole = i64::from(zone.size.w) * i64::from(zone.size.h);
    let shared_area = i64::from(shared.size.w) * i64::from(shared.size.h);
    whole > 0 && shared_area * 100 >= whole * TAKEN
}

impl Windows {
    fn snapped_area(&self, window: WindowId) -> Option<Rect<i32, Logical>> {
        self.snapped(window)
            .map(|snapped| snapped.zone)
            .or_else(|| Some(self.desktop.window(window)?.geometry()))
    }

    fn on_display(&self, output: OutputId) -> impl DoubleEndedIterator<Item = WindowId> + '_ {
        self.desktop.stacking().filter(move |window| {
            self.output_of(*window) == Some(output)
                && self
                    .desktop
                    .window(*window)
                    .is_some_and(|record| record.visibility().shown() && !record.unmanaged())
        })
    }

    fn waiting_windows(
        &self,
        output: OutputId,
        layout: Layout,
        area: Rect<i32, Logical>,
    ) -> Vec<WindowId> {
        let zones: Vec<Rect<i32, Logical>> = layout.zones(area).collect();
        self.on_display(output)
            .rev()
            .filter(|window| {
                !self.enlargements(*window).fullscreen
                    && !self.is_tiled(*window)
                    && self
                        .snapped_area(*window)
                        .is_some_and(|geometry| !zones.iter().any(|zone| covers(geometry, *zone)))
            })
            .collect()
    }

    fn is_zone_occupied(&self, output: OutputId, zone: Rect<i32, Logical>) -> bool {
        self.on_display(output)
            .filter_map(|window| self.snapped_area(window))
            .any(|geometry| covers(geometry, zone))
    }
}

impl WindowsCtx<'_> {
    pub(super) fn offer_snap_assist(
        &mut self,
        output: OutputId,
        area: Rect<i32, Logical>,
        layout: Layout,
    ) {
        if !self.windows.rules().snap.assist {
            return self.dismiss_snap_assist();
        }
        let empty: Vec<Rect<i32, Logical>> = layout
            .zones(area)
            .filter(|zone| !self.windows.is_zone_occupied(output, *zone))
            .collect();
        let Some(target) = empty.first().copied() else {
            return self.dismiss_snap_assist();
        };
        let waiting = self.windows.waiting_windows(output, layout, area);
        let slots = slots(target, shape_of(area.size), waiting.len());
        let cards: Vec<Card> = waiting
            .into_iter()
            .zip(slots)
            .filter_map(|(window, slot)| {
                let home = self.windows.desktop.window(window)?.geometry();
                let from = self
                    .snap
                    .assist()
                    .and_then(|assist| assist.card_rect(window));
                Some(Card::new(
                    window,
                    self.window_title(window),
                    home,
                    slot,
                    from,
                ))
            })
            .collect();
        if cards.is_empty() {
            return self.dismiss_snap_assist();
        }
        self.snap
            .show_assist(Assist::new(output, layout, empty, target, cards));
        self.redraw(output);
    }

    pub(crate) fn snap_assist_hovered(&mut self, at: Point<f64, Logical>) {
        let Some(assist) = self.snap.assist_mut() else {
            return;
        };
        if assist.hover(at) {
            let output = assist.output();
            self.redraw(output);
        }
    }

    pub(crate) fn dismiss_snap_assist(&mut self) {
        let Some(output) = self.snap.assist().map(|assist| assist.output()) else {
            return;
        };
        let desktop = &self.windows.desktop;
        if self
            .snap
            .dismiss_assist(|window| desktop.window(window).map(|record| record.geometry()))
        {
            self.redraw(output);
        }
    }

    pub(crate) fn forget_offered_snap(&mut self, window: WindowId) {
        let offered = self
            .snap
            .assist()
            .is_some_and(|assist| assist.contains_window(window));
        if offered {
            self.dismiss_snap_assist();
            self.windows.session_changed();
        }
    }

    pub(crate) fn snap_escaped(&mut self, pressed: Key) -> bool {
        if pressed != Key::Escape || !self.snap.snap_assist_open() {
            return false;
        }
        self.dismiss_snap_assist();
        true
    }

    pub(crate) fn snap_assist_pressed(&mut self, at: Point<f64, Logical>) -> bool {
        let Some(assist) = self.snap.assist() else {
            return false;
        };
        if !assist.covers(at) {
            self.dismiss_snap_assist();
            return false;
        }
        let (output, layout) = (assist.output(), assist.layout());
        let Some((window, zone)) = assist.card_at_point(at) else {
            self.dismiss_snap_assist();
            return true;
        };
        let flown = assist.card_rect(window);
        let area = self.windows.usable_area(output);
        self.snap.release_offer(window);
        if self.snap_window_to(window, zone, area)
            && let Some(from) = flown
        {
            self.animations.reshape(window, from, zone, None);
        }
        self.offer_snap_assist(output, area, layout);
        true
    }
}

impl Snapping {
    pub(crate) fn snap_assist_covers(&self, at: Point<f64, Logical>) -> bool {
        self.assist().is_some_and(|assist| assist.covers(at))
    }

    pub(crate) fn snap_assist_open(&self) -> bool {
        self.assist().is_some_and(|assist| !assist.is_leaving())
    }
}
