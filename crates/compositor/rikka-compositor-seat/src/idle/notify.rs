use std::time::{Duration, Instant};

use rikka_compositor_protocols::ext_idle_notification_v1::{
    self as protocol, ExtIdleNotificationV1,
};
use rikka_compositor_protocols::ext_idle_notifier_v1::{self as notifier, ExtIdleNotifierV1};

use crate::access::Access;
use crate::idle::Notification;
use crate::ids::{NotificationId, SeatId};

pub(crate) fn idle_notifier<A: Access>(
    state: &mut A,
    _notifier: ExtIdleNotifierV1,
    request: notifier::Request,
) {
    let (id, timeout, seat, respects_inhibitors) = match request {
        notifier::Request::Destroy => return,
        notifier::Request::GetIdleNotification { id, timeout, seat } => (id, timeout, seat, true),
        notifier::Request::GetInputIdleNotification { id, timeout, seat } => {
            (id, timeout, seat, false)
        }
    };
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    let idle = &mut state.seats().idle;
    if idle.last_activity(seat).is_none() {
        idle.touch(seat, Instant::now());
    }
    let notification = idle.notifications.insert(Notification {
        resource: id.clone(),
        seat,
        timeout: Duration::from_millis(u64::from(timeout)),
        respects_inhibitors,
        idle: false,
        timer: None,
    });
    id.set_key(notification);
    if !(respects_inhibitors && idle.inhibited) {
        arm(
            state,
            notification,
            Duration::from_millis(u64::from(timeout)),
        );
    }
}

pub(crate) fn idle_notification<A: Access>(
    _state: &mut A,
    _notification: ExtIdleNotificationV1,
    _request: protocol::Request,
) {
}

pub(crate) fn idle_notification_destroyed<A: Access>(
    state: &mut A,
    notification: ExtIdleNotificationV1,
) {
    let Some(id) = notification.key::<NotificationId>() else {
        return;
    };
    let Some(removed) = state.seats().idle.notifications.remove(id) else {
        return;
    };
    if let Some(timer) = removed.timer {
        state.display().reactor().remove(timer);
    }
}

pub(crate) fn activity<A: Access>(state: &mut A, seat: SeatId) {
    let idle = &mut state.seats().idle;
    idle.touch(seat, Instant::now());
    let inhibited = idle.inhibited;
    let mut rearm = Vec::new();
    for (id, notification) in idle
        .notifications
        .iter_mut()
        .filter(|(_, notification)| notification.seat == seat)
    {
        if notification.idle {
            notification.idle = false;
            notification.resource.resumed();
        }
        let suppressed = notification.respects_inhibitors && inhibited;
        if notification.timer.is_none() && !suppressed {
            rearm.push((id, notification.timeout));
        }
    }
    for (id, timeout) in rearm {
        arm(state, id, timeout);
    }
}

pub(crate) fn set_inhibited<A: Access>(state: &mut A, inhibited: bool) {
    let idle = &mut state.seats().idle;
    if idle.inhibited == inhibited {
        return;
    }
    idle.inhibited = inhibited;
    let mut forgotten = Vec::new();
    let mut rearm = Vec::new();
    for (id, notification) in idle
        .notifications
        .iter_mut()
        .filter(|(_, notification)| notification.respects_inhibitors)
    {
        if inhibited {
            if notification.idle {
                notification.idle = false;
                notification.resource.resumed();
            }
            forgotten.extend(notification.timer.take());
        } else if notification.timer.is_none() {
            rearm.push((id, notification.timeout));
        }
    }
    for timer in forgotten {
        state.display().reactor().remove(timer);
    }
    for (id, timeout) in rearm {
        arm(state, id, timeout);
    }
}

fn arm<A: Access>(state: &mut A, id: NotificationId, delay: Duration) {
    let timer = state
        .display()
        .reactor()
        .after(delay, move |state: &mut A::Host| expire(state, id));
    if let Some(notification) = state.seats().idle.notifications.get_mut(id) {
        notification.timer = Some(timer);
    }
}

fn expire<A: Access>(state: &mut A, id: NotificationId) {
    let idle = &mut state.seats().idle;
    let inhibited = idle.inhibited;
    let Some(notification) = idle.notifications.get(id) else {
        return;
    };
    let last = idle
        .last_activity(notification.seat)
        .unwrap_or_else(Instant::now);
    let Some(notification) = idle.notifications.get_mut(id) else {
        return;
    };
    notification.timer = None;
    if notification.idle || (notification.respects_inhibitors && inhibited) {
        return;
    }
    let elapsed = last.elapsed();
    if elapsed >= notification.timeout {
        notification.idle = true;
        notification.resource.idled();
        return;
    }
    let remaining = notification.timeout - elapsed;
    arm(state, id, remaining);
}
