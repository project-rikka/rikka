use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use rikka_wayland_client::Route;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_presentation::{self, WpPresentation};
use rikka_wayland_client::wp_presentation_feedback::{self, WpPresentationFeedback};
use rustix::time::{ClockId, clock_gettime};

use crate::display::Display;
use crate::error::Result;

const CLOCK_MONOTONIC_ID: u32 = ClockId::Monotonic as u32;
const CLOCK_UNKNOWN: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationFeedback {
    pub key: u64,
    pub presented: Option<PresentationTime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationTime {
    pub timestamp: Instant,
    pub refresh: Option<Duration>,
}

pub struct Presentation {
    manager: WpPresentation,
    feedback: Route,
}

impl Presentation {
    pub fn new(
        display: &Display,
        callback: impl Fn(PresentationFeedback) + Send + Sync + 'static,
    ) -> Result<Presentation> {
        let clock = Arc::new(AtomicU32::new(CLOCK_UNKNOWN));
        let announced = Arc::clone(&clock);
        let clocks = Route::tap::<WpPresentation>(move |_, event| {
            if let wp_presentation::Event::ClockId { clk_id } = event {
                announced.store(clk_id, Ordering::Release);
            }
        });
        let manager: WpPresentation = display.bind_on(&clocks, 1..=2)?;
        let feedback = Route::tap::<WpPresentationFeedback>(move |feedback, event| {
            let Some(key) = feedback.key::<u64>() else {
                return;
            };
            let monotonic = clock.load(Ordering::Acquire) == CLOCK_MONOTONIC_ID;
            if let Some(presented) = presented(event, monotonic) {
                callback(PresentationFeedback { key, presented });
            }
        });
        Ok(Presentation { manager, feedback })
    }

    pub fn feedback(&self, surface: &WlSurface, key: u64) {
        self.manager
            .via(&self.feedback)
            .feedback(surface)
            .set_key(key);
    }
}

impl std::fmt::Debug for Presentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presentation").finish_non_exhaustive()
    }
}

fn presented(
    event: wp_presentation_feedback::Event,
    monotonic: bool,
) -> Option<Option<PresentationTime>> {
    match event {
        wp_presentation_feedback::Event::Presented {
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
            refresh,
            ..
        } => Some(monotonic.then(|| {
            let seconds = (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo);
            PresentationTime {
                timestamp: self::monotonic(Duration::new(seconds, tv_nsec)),
                refresh: (refresh > 0).then(|| Duration::from_nanos(u64::from(refresh))),
            }
        })),
        wp_presentation_feedback::Event::Discarded => Some(None),
        _ => None,
    }
}

fn monotonic(timestamp: Duration) -> Instant {
    let instant = Instant::now();
    let now = clock_gettime(ClockId::Monotonic);
    let now = Duration::new(
        u64::try_from(now.tv_sec).unwrap_or_default(),
        u32::try_from(now.tv_nsec).unwrap_or_default(),
    );
    match now.checked_sub(timestamp) {
        Some(elapsed) => instant.checked_sub(elapsed).unwrap_or(instant),
        None => instant + (timestamp - now),
    }
}
