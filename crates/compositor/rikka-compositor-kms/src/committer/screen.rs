use std::collections::VecDeque;

use rikka_compositor_reactor::{Timestamp, Token};
use rikka_compositor_types::Flip;

use super::command::Setup;
use super::decide::{AsyncSupport, CommitState, Content};
use super::lead_time::LeadTime;
use super::sequence::Sequence;
use super::slots::CursorSlots;
use super::validated::Validated;
use crate::cursor::CursorImage;
use crate::damage::Damage;
use crate::event::{Refresh, Retired};
use crate::frame_commit::{FrameCommit, Modeset, PlaneScanout, PlaneUpdate};
use crate::framebuffer::Framebuffer;
use crate::ids::CrtcId;
use crate::present::{Fence, Present};
use crate::properties::PlaneProps;
use crate::rect::Rect;
use crate::timing::Timing;

#[derive(Debug)]
pub(super) struct Screen {
    pub(super) setup: Setup,
    pub(super) modeset: bool,
    pub(super) vrr_dirty: bool,
    pub(super) vrr_fixed: bool,
    pub(super) queue: VecDeque<Queued>,
    pub(super) in_flight: Option<InFlightCommit>,
    pub(super) front: Option<Frame>,
    pub(super) cursor: Cursor,
    pub(super) deadline: Option<(Token, Timestamp)>,
    pub(super) validated: Validated,
    pub(super) async_refused: Validated,
    pub(super) sequence: Sequence,
    pub(super) lead: LeadTime,
    pub(super) last_vblank: Option<Timestamp>,
    pub(super) last_sequence: u64,
    pub(super) last_async: Option<Timestamp>,
}

#[derive(Debug)]
pub(super) struct Queued {
    pub(super) present: Present,
    pub(super) watch: Option<Token>,
    pub(super) signaled: bool,
}

#[derive(Debug)]
pub(super) struct Frame {
    pub(super) id: u64,
    pub(super) primary: Framebuffer,
    pub(super) zero_copy: bool,
    pub(super) flip: Flip,
}

#[derive(Debug)]
pub(super) struct InFlightCommit {
    pub(super) frame: Option<Frame>,
    pub(super) cursor: Option<Option<CursorImage>>,
    pub(super) asynchronous: bool,
}

#[derive(Debug, Default)]
pub(super) struct Cursor {
    pub(super) image: Option<CursorImage>,
    pub(super) at: Option<(i32, i32)>,
    pub(super) dirty: bool,
    pub(super) shown: Option<CursorImage>,
    pub(super) rejected: bool,
}

impl Screen {
    pub(super) fn new(setup: Setup, cursor: Cursor) -> Self {
        Self {
            setup,
            modeset: true,
            vrr_dirty: false,
            vrr_fixed: false,
            queue: VecDeque::new(),
            in_flight: None,
            front: None,
            cursor: Cursor {
                dirty: true,
                ..cursor
            },
            deadline: None,
            validated: Validated::default(),
            async_refused: Validated::default(),
            sequence: Sequence::default(),
            lead: LeadTime::default(),
            last_vblank: None,
            last_sequence: 0,
            last_async: None,
        }
    }

    pub(super) fn crtc(&self) -> CrtcId {
        self.setup.crtc
    }

    pub(super) fn timing(&self) -> Timing {
        let refresh = self.setup.mode.refresh();
        Timing {
            refresh,
            vrr: self.setup.vrr,
            last_vblank: self.last_vblank,
            sequence: self.last_sequence,
            lead: self.lead.before_vblank(self.setup.mode.vblank(), refresh),
        }
    }

    pub(super) fn publish(&self) {
        self.setup.timeline.publish(self.timing());
    }

    pub(super) fn cursor_plane(&self) -> Option<PlaneProps> {
        self.setup.cursor.filter(|_| !self.cursor.rejected)
    }

    pub(super) fn follow_cursor(&mut self, cursors: &CursorSlots) {
        let Some(at) = cursors.take(self.crtc()) else {
            return;
        };
        if at != self.cursor.at {
            self.cursor.at = at;
            self.cursor.dirty = true;
        }
    }

    fn flips_asynchronously(&self, supported: bool) -> bool {
        supported && !self.modeset && !self.vrr_dirty && self.front.is_some()
    }

    pub(super) fn commit_state(
        &self,
        now: Timestamp,
        support: AsyncSupport,
        moved: bool,
    ) -> CommitState {
        let latest_ready = self.queue.iter().rev().find(|queued| queued.signaled);
        let content = match (latest_ready, self.queue.back()) {
            (Some(ready), _) => Content::Ready {
                not_before: ready.present.not_before,
                flip: ready.present.flip,
            },
            (None, Some(waiting)) => Content::Waiting {
                not_before: waiting.present.not_before,
            },
            (None, None) => Content::None,
        };
        let cursor_dirty = (self.cursor.dirty || moved) && self.cursor_plane().is_some();
        CommitState {
            now,
            timing: self.timing(),
            flip_pending: self.in_flight.is_some(),
            content,
            async_content: self.flips_asynchronously(support.flips) && !cursor_dirty,
            cursor_dirty,
            async_cursor: self.cursor_plane().is_some()
                && self.flips_asynchronously(support.cursor),
            last_async: self.last_async,
            restore: self.modeset && self.front.is_some(),
        }
    }

    pub(super) fn commit(
        &self,
        shown: Option<&Framebuffer>,
        with_cursor: bool,
    ) -> (FrameCommit, Option<Option<CursorImage>>) {
        let setup = &self.setup;
        let mut commit = FrameCommit::new(setup.crtc_props);
        if self.modeset {
            commit.modeset = Some(Modeset {
                connector: setup.connector,
                mode_blob: setup.blob.id(),
                max_bpc: setup.max_bpc,
                signalling: setup.signalling(),
            });
            commit.vrr = Some(setup.vrr);
            commit.overlay = setup.overlay.map(PlaneUpdate::Hide);
        } else if self.vrr_dirty {
            commit.vrr = Some(setup.vrr);
        }
        if let Some(primary) = shown {
            commit.primary = Some(PlaneUpdate::Show(show(setup.primary, primary)));
        }
        let cursor = match with_cursor {
            true => self.cursor_update(),
            false => None,
        };
        let visible = cursor.map(|(update, visible)| {
            commit.cursor = Some(update);
            visible
        });
        (commit, visible)
    }

    fn cursor_update(&self) -> Option<(PlaneUpdate, Option<CursorImage>)> {
        let plane = self.cursor_plane()?;
        Some(match (&self.cursor.image, self.cursor.at) {
            (Some(image), Some((x, y))) => {
                let framebuffer = &image.framebuffer;
                let (width, height) = (framebuffer.width(), framebuffer.height());
                let update = PlaneUpdate::Show(PlaneScanout {
                    plane,
                    framebuffer: framebuffer.id().raw(),
                    format: framebuffer.format(),
                    source: Rect::sized(width, height),
                    destination: Rect::new(x - image.hotspot.0, y - image.hotspot.1, width, height),
                    fence: None,
                    damage: None,
                });
                (update, Some(image.clone()))
            }
            _ => (PlaneUpdate::Hide(plane), None),
        })
    }

    pub(super) fn refresh(&self) -> Refresh {
        let refresh = self.setup.mode.refresh();
        match self.setup.vrr {
            true => Refresh::Variable { shortest: refresh },
            false => Refresh::Fixed(refresh),
        }
    }
}

impl Frame {
    pub(super) fn take(present: Present) -> (Self, Option<Fence>, Damage) {
        let zero_copy = present.zero_copy();
        let frame = Self {
            id: present.frame,
            primary: present.primary,
            zero_copy,
            flip: present.flip,
        };
        (frame, present.fence, present.damage)
    }

    pub(super) fn retired(self, crtc: CrtcId, shown: bool) -> Retired {
        Retired {
            crtc,
            frame: self.id,
            shown,
            primary: self.primary,
        }
    }
}

fn show(plane: PlaneProps, framebuffer: &Framebuffer) -> PlaneScanout {
    let whole = Rect::sized(framebuffer.width(), framebuffer.height());
    PlaneScanout {
        plane,
        framebuffer: framebuffer.id().raw(),
        format: framebuffer.format(),
        source: whole,
        destination: whole,
        fence: None,
        damage: None,
    }
}
