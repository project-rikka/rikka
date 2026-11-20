use std::time::Duration;

use rikka_wayland_client::ext_idle_notification_v1::ExtIdleNotificationV1;
use rikka_wayland_client::ext_idle_notifier_v1::ExtIdleNotifierV1;
use rikka_wayland_client::wl_seat::WlSeat;

use shell_kit::Host;
use shell_kit::hosting::Idles;

#[derive(Debug)]
struct Armed {
    after: Duration,
    notification: ExtIdleNotificationV1,
    idle: bool,
}

#[derive(Debug)]
pub(super) struct Idling {
    idles: Idles,
    notifier: Option<ExtIdleNotifierV1>,
    armed: Vec<Option<Armed>>,
}

impl Idling {
    pub(crate) fn new(idles: Idles, notifier: Option<ExtIdleNotifierV1>) -> Idling {
        Idling {
            idles,
            notifier,
            armed: Vec::new(),
        }
    }

    pub(crate) fn follow(&mut self, seat: Option<&WlSeat>, host: &mut Host) {
        let mut index = 0;
        while let Some(idle) = self.idles.get(index) {
            if self.armed.len() <= index {
                self.armed.resize_with(index + 1, || None);
            }
            let moved = idle.take_moved();
            let timeout = idle.timeout();
            let armed = self.armed[index].as_ref().map(|armed| armed.after);
            if moved || armed != timeout {
                if let Some(armed) = self.armed[index].take() {
                    armed.notification.destroy();
                    if armed.idle {
                        idle.resumed(host);
                    }
                }
                if let (Some(after), Some(notifier), Some(seat)) = (timeout, &self.notifier, seat) {
                    let millis = u32::try_from(after.as_millis()).unwrap_or(u32::MAX);
                    self.armed[index] = Some(Armed {
                        after,
                        notification: notifier
                            .get_idle_notification(millis, seat)
                            .with_key(index as u64),
                        idle: false,
                    });
                }
            }
            index += 1;
        }
    }

    pub(crate) fn idled(&mut self, index: usize, host: &mut Host) {
        self.mark(index, true, host);
    }

    pub(crate) fn resumed(&mut self, index: usize, host: &mut Host) {
        self.mark(index, false, host);
    }

    fn mark(&mut self, index: usize, idle: bool, host: &mut Host) {
        let Some(armed) = self.armed.get_mut(index).and_then(Option::as_mut) else {
            return;
        };
        if armed.idle == idle {
            return;
        }
        armed.idle = idle;
        let Some(timer) = self.idles.get(index) else {
            return;
        };
        match idle {
            true => timer.idled(host),
            false => timer.resumed(host),
        }
    }
}
