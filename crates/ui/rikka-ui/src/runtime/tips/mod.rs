mod bubble;

use std::time::{Duration, Instant};

use rikka_types::Rect;

use crate::view::Tooltip;

pub(crate) use bubble::paint;

const REST: Duration = Duration::from_millis(500);
const BROWSING: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cause {
    Pointer,
    Keys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entrance {
    Grow,
    AtOnce,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aim {
    pub(crate) id: u64,
    pub(crate) part: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Waiting {
        aim: Aim,
        due: Instant,
        cause: Cause,
        entrance: Entrance,
    },
    Showing {
        aim: Aim,
        cause: Cause,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bubble {
    pub(crate) tooltip: Tooltip,
    pub(crate) anchor: Rect,
    pub(crate) open: bool,
    pub(crate) entrance: Option<Entrance>,
}

#[derive(Default)]
pub(crate) struct Tips {
    phase: Phase,
    pointed: Option<Aim>,
    hushed: Option<Aim>,
    browsing_until: Option<Instant>,
    bubble: Option<Bubble>,
    changed: bool,
}

impl Tips {
    pub(crate) fn due(&self) -> Option<Instant> {
        match self.phase {
            Phase::Waiting { due, .. } => Some(due),
            Phase::Idle | Phase::Showing { .. } => None,
        }
    }

    pub(crate) fn unsettled(&self) -> bool {
        self.changed
    }

    pub(crate) fn point(&mut self, target: Option<Aim>, moved: bool, now: Instant) {
        if target != self.pointed {
            self.pointed = target;
            if self.hushed != target {
                self.hushed = None;
            }
            let ends = match self.cause() {
                Some(Cause::Pointer) => true,
                Some(Cause::Keys) => moved && target.is_some(),
                None => false,
            };
            if ends {
                self.close(Some(now));
            }
        }
        let (true, Some(aim)) = (moved, target) else {
            return;
        };
        match &mut self.phase {
            Phase::Waiting {
                aim: waiting,
                due,
                cause: Cause::Pointer,
                entrance: Entrance::Grow,
            } if *waiting == aim => *due = now + REST,
            Phase::Idle => self.arm(aim, Cause::Pointer, now),
            Phase::Waiting { .. } | Phase::Showing { .. } => {}
        }
    }

    pub(crate) fn focus(&mut self, target: Option<Aim>, now: Instant) {
        if let Some(aim) = target {
            self.arm(aim, Cause::Keys, now);
        }
    }

    pub(crate) fn dismiss(&mut self) {
        self.hushed = self.pointed;
        self.close(None);
    }

    pub(crate) fn leave(&mut self) {
        self.pointed = None;
        self.hushed = None;
        self.close(None);
    }

    pub(crate) fn settle(
        &mut self,
        now: Instant,
        lookup: impl Fn(Aim) -> Option<(Tooltip, Rect)>,
    ) -> bool {
        match self.phase {
            Phase::Waiting {
                aim,
                due,
                cause,
                entrance,
            } if due <= now => match lookup(aim) {
                Some((tooltip, anchor)) => {
                    self.phase = Phase::Showing { aim, cause };
                    self.bubble = Some(Bubble {
                        tooltip,
                        anchor,
                        open: true,
                        entrance: Some(entrance),
                    });
                    self.changed = true;
                }
                None => self.phase = Phase::Idle,
            },
            Phase::Showing { aim, .. } => match lookup(aim) {
                Some((tooltip, anchor)) => self.follow(tooltip, anchor),
                None => {
                    self.phase = Phase::Idle;
                    self.changed |= self.bubble.take().is_some();
                }
            },
            Phase::Idle | Phase::Waiting { .. } => {}
        }
        std::mem::take(&mut self.changed)
    }

    pub(crate) fn painted(&mut self) -> Option<Bubble> {
        let bubble = self.bubble.as_mut()?;
        let painted = bubble.clone();
        bubble.entrance = None;
        Some(painted)
    }

    fn cause(&self) -> Option<Cause> {
        match self.phase {
            Phase::Idle => None,
            Phase::Waiting { cause, .. } | Phase::Showing { cause, .. } => Some(cause),
        }
    }

    fn arm(&mut self, aim: Aim, cause: Cause, now: Instant) {
        if self.hushed == Some(aim) && cause == Cause::Pointer {
            return;
        }
        let browsing = self.browsing_until.is_some_and(|until| now < until);
        self.phase = Phase::Waiting {
            aim,
            due: match browsing {
                true => now,
                false => now + REST,
            },
            cause,
            entrance: match browsing {
                true => Entrance::AtOnce,
                false => Entrance::Grow,
            },
        };
    }

    fn close(&mut self, browsing_from: Option<Instant>) {
        let was_showing = matches!(self.phase, Phase::Showing { .. });
        self.phase = Phase::Idle;
        self.browsing_until = match was_showing {
            true => browsing_from.map(|now| now + BROWSING),
            false => None,
        };
        if let Some(bubble) = self.bubble.as_mut().filter(|bubble| bubble.open) {
            bubble.open = false;
            bubble.entrance = None;
            self.changed = true;
        }
    }

    fn follow(&mut self, tooltip: Tooltip, anchor: Rect) {
        let Some(bubble) = self.bubble.as_mut() else {
            return;
        };
        if bubble.anchor != anchor || bubble.tooltip != tooltip {
            bubble.tooltip = tooltip;
            bubble.anchor = anchor;
            self.changed = true;
        }
    }
}
