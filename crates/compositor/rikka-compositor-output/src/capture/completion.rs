use rikka_compositor_geometry::{Rect, Region};
use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::FailureReason;
use rikka_compositor_protocols::wl_output;

use super::constraints;
use super::store::{Captures, Phase};
use super::types::{CaptureSource, Constraints, FrameReady};
use crate::ids::{CaptureFrameId, CaptureSessionId};

impl<T: Copy + PartialEq> Captures<T> {
    pub fn ready(&mut self, frame: CaptureFrameId, ready: FrameReady) -> bool {
        let Some(entry) = self.frames.get_mut(frame) else {
            return false;
        };
        let Phase::Capturing { session, size } = entry.phase else {
            return false;
        };
        let whole = Rect::from_size(size);
        let first = self
            .sessions
            .get_mut(session)
            .is_none_or(|session| !std::mem::replace(&mut session.delivered, true));
        let damage = if first {
            Region::from(whole)
        } else {
            ready.damage.intersect(whole)
        };
        let resource = &entry.resource;
        resource.transform(
            wl_output::Transform::try_from(ready.transform.to_wayland())
                .unwrap_or(wl_output::Transform::Normal),
        );
        for rect in damage.iter() {
            resource.damage(rect.loc.x, rect.loc.y, rect.size.w, rect.size.h);
        }
        let seconds = ready.presented.as_secs();
        resource.presentation_time(
            (seconds >> 32) as u32,
            (seconds & u64::from(u32::MAX)) as u32,
            ready.presented.subsec_nanos(),
        );
        resource.ready();
        entry.phase = Phase::Finished;
        true
    }

    pub fn fail(&mut self, frame: CaptureFrameId, reason: FailureReason) -> bool {
        let Some(entry) = self.frames.get_mut(frame) else {
            return false;
        };
        if !matches!(entry.phase, Phase::Capturing { .. }) {
            return false;
        }
        entry.resource.failed(reason);
        entry.phase = Phase::Finished;
        true
    }

    pub fn update_constraints(&mut self, session: CaptureSessionId, next: Constraints) {
        let Some(entry) = self.sessions.get_mut(session) else {
            return;
        };
        if entry.stopped || entry.constraints.as_ref() == Some(&next) {
            return;
        }
        constraints::send(&entry.resource, &next);
        entry.constraints = Some(next);
        if let Some(frame) = entry.frame {
            self.fail(frame, FailureReason::BufferConstraints);
        }
    }

    pub fn stop(&mut self, session: CaptureSessionId) {
        let Some(entry) = self.sessions.get_mut(session) else {
            return;
        };
        if std::mem::replace(&mut entry.stopped, true) {
            return;
        }
        entry.resource.stopped();
        if let Some(frame) = entry.frame {
            self.fail(frame, FailureReason::Stopped);
        }
    }

    pub fn stop_source(&mut self, source: CaptureSource<T>) {
        let stopping: Vec<CaptureSessionId> = self
            .sessions
            .iter()
            .filter(|(_, entry)| entry.info.source == source)
            .map(|(id, _)| id)
            .collect();
        for session in stopping {
            self.stop(session);
        }
    }
}
