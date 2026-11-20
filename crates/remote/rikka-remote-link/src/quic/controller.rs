use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

use quinn::congestion::{Controller, ControllerFactory};

const WINDOW: u64 = 64 << 20;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ApplicationPaced;

impl ControllerFactory for ApplicationPaced {
    fn build(self: Arc<Self>, _now: Instant, _current_mtu: u16) -> Box<dyn Controller> {
        Box::new(ApplicationPaced)
    }
}

impl Controller for ApplicationPaced {
    fn on_congestion_event(
        &mut self,
        _now: Instant,
        _sent: Instant,
        _is_persistent_congestion: bool,
        _lost_bytes: u64,
    ) {
    }

    fn on_mtu_update(&mut self, _new_mtu: u16) {}

    fn window(&self) -> u64 {
        WINDOW
    }

    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(*self)
    }

    fn initial_window(&self) -> u64 {
        WINDOW
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
