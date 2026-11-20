use super::shot::{Shot, Viewpoint, preset_transforms};
use crate::clip::{Clip, ClipId};
use crate::project::Project;
use crate::visual::{Appearance, Visual};

impl Project {
    #[must_use]
    pub fn shot_as<'a>(
        &self,
        clip: &Clip,
        frame: i64,
        visual: Visual,
        view: Viewpoint,
        restyled: impl Fn(ClipId) -> Option<&'a Appearance>,
    ) -> Option<Shot> {
        let (layer, _) = self.locate(clip.id())?;
        let cameras = self.cameras_at(frame, view, &restyled);
        let transforms = preset_transforms(clip, frame, &visual);
        Some(self.shoot(layer, visual, transforms, &cameras, view))
    }
}
