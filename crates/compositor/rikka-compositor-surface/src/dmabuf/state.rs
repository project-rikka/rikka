use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use rikka_compositor_types::Format;
use slotmap::SlotMap;

use super::feedback::DmabufFeedback;
use crate::ids::SurfaceId;
use crate::surface::Surface;
use crate::surfaces::Surfaces;

#[derive(Debug, Default)]
pub(crate) struct DmabufState {
    pub(crate) default: Option<DmabufFeedback>,
    pub(crate) objects: Vec<ZwpLinuxDmabufFeedbackV1>,
    supported: Vec<Format>,
}

impl DmabufState {
    pub(crate) fn supports(&self, format: &Format) -> bool {
        self.supported.binary_search(format).is_ok()
    }

    pub(crate) fn rebuild(&mut self, surfaces: &SlotMap<SurfaceId, Surface>) {
        let mut offered: Vec<&DmabufFeedback> = Vec::new();
        for feedback in self.default.iter().chain(
            surfaces
                .values()
                .filter_map(|surface| surface.hints.dmabuf.as_ref()),
        ) {
            if !offered
                .iter()
                .any(|offered_feedback| offered_feedback.same(feedback))
            {
                offered.push(feedback);
            }
        }
        self.supported.clear();
        for feedback in offered {
            self.supported.extend(feedback.sorted());
        }
        self.supported.sort_unstable();
        self.supported.dedup();
    }
}

impl Surfaces {
    pub fn set_dmabuf_feedback(&mut self, feedback: DmabufFeedback) {
        if self
            .dmabuf
            .default
            .as_ref()
            .is_some_and(|current| current.same(&feedback))
        {
            return;
        }
        self.dmabuf.objects.retain(ZwpLinuxDmabufFeedbackV1::alive);
        for object in &self.dmabuf.objects {
            feedback.send(object);
        }
        for surface in self.surfaces.values_mut() {
            if surface.hints.dmabuf.is_none() {
                surface
                    .extensions
                    .dmabuf_feedback
                    .retain(ZwpLinuxDmabufFeedbackV1::alive);
                for object in &surface.extensions.dmabuf_feedback {
                    feedback.send(object);
                }
            }
        }
        self.dmabuf.default = Some(feedback);
        self.dmabuf.rebuild(&self.surfaces);
    }

    pub fn set_surface_dmabuf_feedback(
        &mut self,
        surface: SurfaceId,
        feedback: Option<DmabufFeedback>,
    ) {
        let default = self.dmabuf.default.clone();
        let Some(target) = self.surfaces.get_mut(surface) else {
            return;
        };
        let advised = match (&target.hints.dmabuf, &feedback) {
            (Some(before), Some(after)) => !before.same(after),
            (None, None) => false,
            _ => true,
        };
        let before = target.hints.dmabuf.clone().or_else(|| default.clone());
        target.hints.dmabuf = feedback;
        let after = target.hints.dmabuf.clone().or(default);
        if advised {
            self.dmabuf.rebuild(&self.surfaces);
        }
        let Some(after) = after else {
            return;
        };
        if before.is_some_and(|before| before.same(&after)) {
            return;
        }
        let Some(target) = self.surfaces.get_mut(surface) else {
            return;
        };
        target
            .extensions
            .dmabuf_feedback
            .retain(ZwpLinuxDmabufFeedbackV1::alive);
        for object in &target.extensions.dmabuf_feedback {
            after.send(object);
        }
    }

    pub(crate) fn dmabuf_feedback_for(&self, surface: Option<SurfaceId>) -> Option<DmabufFeedback> {
        surface
            .and_then(|surface| self.surfaces.get(surface))
            .and_then(|surface| surface.hints.dmabuf.clone())
            .or_else(|| self.dmabuf.default.clone())
    }
}
