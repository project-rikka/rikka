use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Flip;

use crate::timing::Timing;

const CURSOR_MOVES_PER_REFRESH: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
    None,
    Waiting {
        not_before: Option<Timestamp>,
    },
    Ready {
        not_before: Option<Timestamp>,
        flip: Flip,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AsyncSupport {
    pub(crate) flips: bool,
    pub(crate) cursor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommitState {
    pub(crate) now: Timestamp,
    pub(crate) timing: Timing,
    pub(crate) flip_pending: bool,
    pub(crate) content: Content,
    pub(crate) async_content: bool,
    pub(crate) cursor_dirty: bool,
    pub(crate) async_cursor: bool,
    pub(crate) last_async: Option<Timestamp>,
    pub(crate) restore: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    Wait,
    At(Timestamp),
    Now,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WakeReason {
    Requested,
    Deadline,
}

pub(crate) fn arrived(decision: Decision) -> Decision {
    match decision {
        Decision::At(_) => Decision::Now,
        decision => decision,
    }
}

impl CommitState {
    fn paced(&self) -> Option<(Timestamp, Timestamp)> {
        match self.timing.vrr {
            true => None,
            false => self.timing.next_deadline(self.now),
        }
    }

    pub(crate) fn target(&self) -> Option<Timestamp> {
        self.paced().map(|(vblank, _)| vblank)
    }
}

pub(crate) fn decide(commit_state: &CommitState) -> Decision {
    if commit_state.flip_pending {
        return Decision::Wait;
    }
    if let Content::Ready {
        not_before,
        flip: Flip::Async,
    } = commit_state.content
        && commit_state.async_content
    {
        return match not_before {
            Some(at) if at > commit_state.now => Decision::At(at),
            _ => Decision::Now,
        };
    }
    let other = commit_state.cursor_dirty || commit_state.restore;
    if commit_state.content == Content::None && !other {
        return Decision::Wait;
    }
    if commit_state.content == Content::None
        && commit_state.cursor_dirty
        && commit_state.async_cursor
        && !commit_state.restore
    {
        let settled = commit_state
            .last_async
            .map(|moved| moved + commit_state.timing.refresh / CURSOR_MOVES_PER_REFRESH);
        return match settled {
            Some(settled) if settled > commit_state.now => Decision::At(settled),
            _ => Decision::Now,
        };
    }
    let Some((vblank, deadline)) = commit_state.paced() else {
        return unpaced(commit_state, other);
    };
    let timing = &commit_state.timing;
    let content_due = match commit_state.content {
        Content::None => None,
        Content::Waiting { not_before } | Content::Ready { not_before, .. } => Some(
            not_before
                .map(|at| at.saturating_sub(timing.refresh / 2))
                .filter(|target| *target > vblank)
                .and_then(|target| timing.next_vblank(target))
                .map_or(deadline, |later| later - timing.lead),
        ),
    };
    let due = [content_due, other.then_some(deadline)]
        .into_iter()
        .flatten()
        .min();
    match due {
        Some(due) if due <= commit_state.now => Decision::Now,
        Some(due) => Decision::At(due),
        None => Decision::Wait,
    }
}

pub(crate) fn eligible(
    not_before: Option<Timestamp>,
    flip: Flip,
    commit_state: &CommitState,
) -> bool {
    let Some(not_before) = not_before else {
        return true;
    };
    let tears = flip == Flip::Async && commit_state.async_content;
    match commit_state.target().filter(|_| !tears) {
        Some(vblank) => not_before.saturating_sub(commit_state.timing.refresh / 2) <= vblank,
        None => not_before <= commit_state.now,
    }
}

fn unpaced(commit_state: &CommitState, other: bool) -> Decision {
    if other {
        return Decision::Now;
    }
    match commit_state.content {
        Content::Ready {
            not_before: Some(at),
            ..
        } if at > commit_state.now => Decision::At(at),
        Content::Ready { .. } => Decision::Now,
        Content::Waiting { .. } | Content::None => Decision::Wait,
    }
}
