use std::time::{Duration, Instant};

use rikka_compositor_desktop::{WindowId, WindowKind};

use super::decoration::{Decoration, client_configure};
use crate::context::{WindowsCtx, windows_ctx};
use crate::state::Compositor;

const ACK_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureSent {
    Done,
    Waiting { until: Instant },
}

impl WindowsCtx<'_> {
    pub(crate) fn flush_configures(&mut self) {
        let pending = std::mem::take(&mut self.windows.pending_configures);
        let mut due: Option<Instant> = None;
        for window in pending {
            if let ConfigureSent::Waiting { until } = self.send_configure(window) {
                due = Some(due.map_or(until, |due| due.min(until)));
                self.windows.pending_configures.push(window);
            }
        }
        let Some(due) = due else {
            return;
        };
        if self.windows.patience.is_some_and(|(armed, _)| armed <= due) {
            return;
        }
        if let Some((_, token)) = self.windows.patience.take() {
            self.core.reactor.remove(token);
        }
        let token = self.core.reactor.timer(due, |compositor: &mut Compositor| {
            windows_ctx!(compositor).ack_timeout_expired()
        });
        self.windows.patience = Some((due, token));
    }

    fn ack_timeout_expired(&mut self) {
        self.windows.patience = None;
        self.flush_configures();
    }

    fn send_configure(&mut self, window: WindowId) -> ConfigureSent {
        let Some(record) = self.windows.desktop.window(window) else {
            return ConfigureSent::Done;
        };
        let WindowKind::Xdg(toplevel) = record.kind() else {
            return ConfigureSent::Done;
        };
        let frame = record.frame();
        let Some(policy) = self.windows.desktop.policy(window) else {
            return ConfigureSent::Done;
        };
        let decoration = match policy.decorated {
            Some(_) => Decoration::Server,
            None => Decoration::Client,
        };
        let configure = client_configure(policy.configure, frame, decoration);
        let patient_until = policy
            .configured_at
            .map(|configured| configured + ACK_TIMEOUT)
            .filter(|until| Instant::now() < *until);
        let Some(state) = self.shell.toplevel(toplevel) else {
            return ConfigureSent::Done;
        };
        if !state.initial_configure_sent() {
            return ConfigureSent::Done;
        }
        if state.awaiting_ack()
            && let Some(until) = patient_until
        {
            return ConfigureSent::Waiting { until };
        }
        if state.latest() == Some(&configure) {
            return ConfigureSent::Done;
        }
        if self.shell.configure_toplevel(toplevel, configure).is_some()
            && let Some(policy) = self.windows.desktop.policy_mut(window)
        {
            policy.configured_at = Some(Instant::now());
        }
        ConfigureSent::Done
    }
}
