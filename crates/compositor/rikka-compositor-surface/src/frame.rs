use std::time::Duration;

use crate::damage::SurfaceDamage;
use crate::feedback::Feedback;
use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;
use crate::tree::PlacedSurface;

impl Surfaces {
    pub fn frame_done(&mut self, root: SurfaceId, time: Duration) {
        let members = self.members_of(root);
        for surface in &members {
            self.frame_done_on(*surface, time);
        }
        self.members = members;
    }

    pub fn frame_done_for(&mut self, placed: &[PlacedSurface], time: Duration) {
        for placed in placed {
            self.frame_done_on(placed.surface, time);
        }
    }

    pub fn take_feedback_for(&mut self, placed: &[PlacedSurface]) -> Feedback {
        let mut feedback = Feedback::default();
        for placed in placed {
            self.take_feedback_of(placed.surface, &mut feedback);
        }
        feedback
    }

    pub fn take_damage_into(&mut self, surface: SurfaceId, damage: &mut SurfaceDamage) {
        damage.buffer.clear();
        damage.surface.clear();
        if let Some(surface) = self.surfaces.get_mut(surface) {
            std::mem::swap(&mut surface.current.damage, damage);
        }
    }

    fn members_of(&mut self, root: SurfaceId) -> Vec<SurfaceId> {
        let mut members = std::mem::take(&mut self.members);
        members.clear();
        members.extend(self.tree(root).map(|placed| placed.surface));
        members
    }

    fn frame_done_on(&mut self, surface: SurfaceId, time: Duration) {
        let millis = time.as_millis() as u32;
        if let Some(surface) = self.surfaces.get_mut(surface) {
            for callback in surface.current.frame_callbacks.drain(..) {
                callback.done(millis);
            }
        }
    }

    fn take_feedback_of(&mut self, surface: SurfaceId, feedback: &mut Feedback) {
        if let Some(surface) = self.surfaces.get_mut(surface) {
            feedback.take_from(&mut surface.current.feedback);
        }
    }
}
