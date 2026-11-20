mod inhibit;
mod notify;

use std::time::{Duration, Instant};

use rikka_compositor_protocols::ext_idle_notification_v1::ExtIdleNotificationV1;
use rikka_compositor_protocols::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1;
use rikka_compositor_reactor::Token;
use rikka_compositor_surface::SurfaceId;
use slotmap::SlotMap;

use crate::ids::{NotificationId, SeatId};

pub(crate) use inhibit::{
    idle_inhibit_manager, idle_inhibitor, idle_inhibitor_destroyed, inhibitors_surface_gone,
};
pub(crate) use notify::{
    activity, idle_notification, idle_notification_destroyed, idle_notifier, set_inhibited,
};

#[derive(Debug)]
pub(crate) struct Notification {
    pub(crate) resource: ExtIdleNotificationV1,
    pub(crate) seat: SeatId,
    pub(crate) timeout: Duration,
    pub(crate) respects_inhibitors: bool,
    pub(crate) idle: bool,
    pub(crate) timer: Option<Token>,
}

#[derive(Debug)]
pub(crate) struct Inhibitor {
    pub(crate) resource: ZwpIdleInhibitorV1,
    pub(crate) surface: SurfaceId,
}

#[derive(Debug, Default)]
pub(crate) struct Idle {
    pub(crate) notifications: SlotMap<NotificationId, Notification>,
    pub(crate) inhibitors: Vec<Inhibitor>,
    pub(crate) inhibited: bool,
    pub(crate) last_activity: Vec<(SeatId, Instant)>,
}

impl Idle {
    pub(crate) fn inhibitors(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.inhibitors.iter().map(|inhibitor| inhibitor.surface)
    }

    pub(crate) fn last_activity(&self, seat: SeatId) -> Option<Instant> {
        self.last_activity
            .iter()
            .find(|(seat_id, _)| *seat_id == seat)
            .map(|(_, at)| *at)
    }

    pub(crate) fn touch(&mut self, seat: SeatId, at: Instant) {
        match self
            .last_activity
            .iter_mut()
            .find(|(seat_id, _)| *seat_id == seat)
        {
            Some((_, last)) => *last = at,
            None => self.last_activity.push((seat, at)),
        }
    }
}
