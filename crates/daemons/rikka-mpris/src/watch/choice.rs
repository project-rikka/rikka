use std::time::Instant;

use crate::track::Status;
use crate::watch::player::{Player, PlayerId};

pub(crate) fn activity(previous: Option<&Player>, read: &mut Player, now: Instant) {
    let Some(previous) = previous else {
        read.active_at = read.is_playing().then_some(now);
        return;
    };
    let started = read.is_playing() && !previous.is_playing();
    let moved_on = read.is_playing()
        && previous.track.as_ref().map(|track| &track.id)
            != read.track.as_ref().map(|track| &track.id);
    read.active_at = match started || moved_on {
        true => Some(now),
        false => previous.active_at,
    };
}

pub(crate) fn active<'a>(players: impl Iterator<Item = &'a Player>) -> Option<PlayerId> {
    players
        .filter(|player| player.status != Status::Stopped)
        .max_by(|left, right| {
            (left.is_playing(), left.active_at)
                .cmp(&(right.is_playing(), right.active_at))
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|player| player.id.clone())
}
