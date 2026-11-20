use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use rikka_compositor_protocols::xdg_wm_base::XdgWmBase;
use rikka_compositor_server::DisplayHandle;

use crate::host::ShellHost;
use crate::xdg::ping;

type Serials = Rc<dyn Fn() -> u32>;
type PingTimer = Rc<dyn Fn(Duration, XdgWmBase, u32)>;

pub(crate) struct Clock {
    serials: Serials,
    ping_timer: PingTimer,
}

impl Clock {
    pub(crate) fn new<S: ShellHost>(display: &DisplayHandle<S>) -> Self {
        let serials = display.clone();
        let reactor = display.reactor().clone();
        Self {
            serials: Rc::new(move || serials.next_serial()),
            ping_timer: Rc::new(move |patience, wm_base, serial| {
                reactor.after(patience, move |state: &mut S| {
                    ping::expired(state, &wm_base, serial);
                });
            }),
        }
    }

    pub(crate) fn serial(&self) -> u32 {
        (self.serials)()
    }

    pub(crate) fn expect_pong(&self, patience: Duration, wm_base: XdgWmBase, serial: u32) {
        (self.ping_timer)(patience, wm_base, serial);
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Clock").finish_non_exhaustive()
    }
}
