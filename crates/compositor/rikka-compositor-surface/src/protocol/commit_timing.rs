use std::time::Duration;

use rikka_compositor_protocols::wp_commit_timer_v1::{self, WpCommitTimerV1};
use rikka_compositor_protocols::wp_commit_timing_manager_v1::{self, WpCommitTimingManagerV1};

use super::extension::{Claim, claim, target};
use crate::host::SurfaceHost;

const NANOS_PER_SECOND: u32 = 1_000_000_000;

pub(crate) fn manager<S: SurfaceHost>(
    state: &mut S,
    manager: WpCommitTimingManagerV1,
    request: wp_commit_timing_manager_v1::Request,
) {
    let wp_commit_timing_manager_v1::Request::GetTimer { id, surface } = request else {
        return;
    };
    match claim(state, &surface, |extensions| {
        !std::mem::replace(&mut extensions.timer, true)
    }) {
        Claim::Claimed(surface) => id.set_key(surface),
        Claim::Occupied => {
            manager.post_error(wp_commit_timing_manager_v1::Error::CommitTimerExists)
        }
        Claim::Missing => {}
    }
}

pub(crate) fn request<S: SurfaceHost>(
    state: &mut S,
    timer: WpCommitTimerV1,
    request: wp_commit_timer_v1::Request,
) {
    let wp_commit_timer_v1::Request::SetTimestamp {
        tv_sec_hi,
        tv_sec_lo,
        tv_nsec,
    } = request
    else {
        return;
    };
    let Some(surface) = target(state, &timer) else {
        timer.post_error(wp_commit_timer_v1::Error::SurfaceDestroyed);
        return;
    };
    if tv_nsec >= NANOS_PER_SECOND {
        timer.post_error(wp_commit_timer_v1::Error::InvalidTimestamp);
        return;
    }
    let constraints = &mut surface.pending.update.constraints;
    if constraints.target.is_some() {
        timer.post_error(wp_commit_timer_v1::Error::TimestampExists);
        return;
    }
    let seconds = (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo);
    constraints.target = Some(Duration::new(seconds, tv_nsec));
}

pub(crate) fn destroyed<S: SurfaceHost>(state: &mut S, timer: WpCommitTimerV1) {
    if let Some(surface) = target(state, &timer) {
        surface.extensions.timer = false;
    }
}
