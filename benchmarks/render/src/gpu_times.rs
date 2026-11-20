use rikka_sync::Mutex;
use std::sync::Arc;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;

use crate::allocations::uncounted;

const PASSES: &str = "rikka_renderer::passes";

#[derive(Clone, Debug)]
pub struct Pass {
    pub frame: u64,
    pub name: String,
    pub micros: f64,
}

#[derive(Clone, Debug, Default)]
pub struct GpuTimes {
    collected: Arc<Mutex<Vec<Pass>>>,
}

impl GpuTimes {
    pub fn install() -> GpuTimes {
        let times = GpuTimes::default();
        let listening = Listening {
            collected: Arc::clone(&times.collected),
        }
        .with_filter(Targets::new().with_target(PASSES, Level::DEBUG));
        tracing_subscriber::registry().with(listening).init();
        times
    }

    pub fn take(&self, first: u64, last: u64) -> Vec<Pass> {
        let mut collected = self.collected.lock();
        let (taken, remaining) = collected
            .drain(..)
            .partition(|pass| (first..=last).contains(&pass.frame));
        *collected = remaining;
        taken
    }
}

struct Listening {
    collected: Arc<Mutex<Vec<Pass>>>,
}

impl<S: Subscriber> Layer<S> for Listening {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        uncounted(|| {
            let mut pass = Pass {
                frame: 0,
                name: String::new(),
                micros: 0.0,
            };
            event.record(&mut pass);
            self.collected.lock().push(pass);
        });
    }
}

impl Visit for Pass {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "frame" {
            self.frame = value;
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "micros" {
            self.micros = value;
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "pass" {
            self.name = value.to_owned();
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "pass" {
            self.name = format!("{value:?}").trim_matches('"').to_owned();
        }
    }
}
