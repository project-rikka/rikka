use std::time::{Duration, Instant};

use rikka_compositor_reactor::{Timestamp, Token};
use rikka_compositor_shell::{PopupParent, ToplevelId};
use rikka_compositor_surface::{PlacedSurface, Presentation, PresentationKind, Refresh, SurfaceId};
use rikka_compositor_types::OutputId;
use rikka_picture::{RefreshRate, Tick};

use super::state::State;
use crate::view::ViewId;

const HIDDEN_FRAME_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct Pacing {
    hidden_timer: Option<Token>,
    sequence: u64,
}

impl State {
    pub(crate) fn request_frame(&mut self, surface: SurfaceId) {
        let Some(toplevel) = self.toplevel_of(surface) else {
            return;
        };
        let view = self.owner_of(toplevel);
        match view.and_then(|view| self.views.records.get_mut(&view)) {
            Some(record) => {
                record.frame_pending = true;
                record.wake_pending = true;
            }
            None => {
                if let Some(tracked) = self.views.toplevels.get_mut(toplevel) {
                    tracked.frame_pending = true;
                }
            }
        }
        self.arm_hidden_timer();
    }

    pub(crate) fn request_refresh(&mut self, key: OutputId) {
        if let Some(record) = self
            .views
            .records
            .values_mut()
            .find(|record| record.pacing == key)
        {
            record.frame_pending = true;
            record.wake_pending = true;
        }
        self.arm_hidden_timer();
    }

    pub(crate) fn wake_pending_views(&mut self) {
        for record in self.views.records.values_mut() {
            if std::mem::take(&mut record.wake_pending) && record.desired.visible {
                record.shared.wake();
            }
        }
    }

    pub(crate) fn on_tick(&mut self, view: ViewId, tick: Tick) {
        if let Some(record) = self.record(view) {
            record.last_presented = Some(Instant::now());
        }
        self.tick_view(
            view,
            Timestamp::from_instant(tick.next()).as_monotonic(),
            tick.period,
        );
    }

    fn tick_view(&mut self, view: ViewId, next: Duration, period: Duration) {
        let Some(record) = self.record(view) else {
            return;
        };
        record.frame_pending = false;
        let key = record.pacing;
        let toplevel = record.toplevel;
        if let Some(toplevel) = toplevel {
            self.tick_toplevel(toplevel, period);
        }
        rikka_compositor_surface::refreshed(self, key, next);
    }

    fn tick_toplevel(&mut self, toplevel: ToplevelId, period: Duration) {
        let members = self.members(toplevel);
        let now = Timestamp::now().as_monotonic();
        self.surfaces.frame_done_for(&members, now);
        let feedback = self.surfaces.take_feedback_for(&members);
        self.pacing.sequence += 1;
        let presentation = Presentation {
            time: now,
            refresh: Refresh::Fixed(period),
            sequence: self.pacing.sequence,
            flags: PresentationKind::VSYNC,
        };
        let outputs = &self.outputs;
        let output = self.output;
        feedback.presented(&presentation, |client| {
            outputs.resources(output, client).collect::<Vec<_>>()
        });
    }

    fn members(&self, root: ToplevelId) -> Vec<PlacedSurface> {
        let family: Vec<ToplevelId> = self
            .shell
            .toplevels()
            .map(|(toplevel, _)| toplevel)
            .filter(|toplevel| self.root_of(*toplevel) == Some(root))
            .collect();
        let mut members = Vec::new();
        for toplevel in &family {
            if let Some(top) = self.shell.toplevel(*toplevel) {
                members.extend(self.surfaces.tree(top.surface()));
            }
        }
        for (popup, state) in self.shell.popups() {
            if let Some(PopupParent::Toplevel(toplevel)) = self.shell.popup_root(popup)
                && family.contains(&toplevel)
            {
                members.extend(self.surfaces.tree(state.surface()));
            }
        }
        members
    }

    fn arm_hidden_timer(&mut self) {
        if self.pacing.hidden_timer.is_some() {
            return;
        }
        let token = self
            .display
            .reactor()
            .after(HIDDEN_FRAME_INTERVAL, |state: &mut State| {
                state.hidden_timer_fired()
            });
        self.pacing.hidden_timer = Some(token);
    }

    fn hidden_timer_fired(&mut self) {
        self.pacing.hidden_timer = None;
        let now = Instant::now();
        let stale: Vec<ViewId> = self
            .views
            .records
            .iter()
            .filter(|(_, record)| {
                record.frame_pending
                    && (!record.desired.visible
                        || record
                            .last_presented
                            .is_none_or(|presented| now - presented >= HIDDEN_FRAME_INTERVAL))
            })
            .map(|(view, _)| *view)
            .collect();
        let unattached: Vec<ToplevelId> = self
            .views
            .toplevels
            .iter()
            .filter(|(_, tracked)| tracked.view.is_none() && tracked.frame_pending)
            .map(|(toplevel, _)| toplevel)
            .collect();
        let next = Timestamp::now().as_monotonic() + HIDDEN_FRAME_INTERVAL;
        let period = RefreshRate::published_period();
        for view in stale {
            self.tick_view(view, next, period);
        }
        for toplevel in unattached {
            if let Some(tracked) = self.views.toplevels.get_mut(toplevel) {
                tracked.frame_pending = false;
            }
            self.tick_toplevel(toplevel, period);
        }
        rikka_compositor_surface::refreshed(self, self.output, next);
        let pending = self
            .views
            .records
            .values()
            .any(|record| record.frame_pending)
            || self
                .views
                .toplevels
                .values()
                .any(|tracked| tracked.frame_pending);
        if pending {
            self.arm_hidden_timer();
        }
    }
}
