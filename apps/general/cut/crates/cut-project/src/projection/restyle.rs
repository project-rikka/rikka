use super::shot::{Shot, Viewpoint, ordered, preset_transforms};
use crate::animation_presets::PresetTransforms;
use crate::clip::{Clip, ClipId};
use crate::project::Project;
use crate::visual::{Appearance, Visual};

impl Project {
    #[must_use]
    pub fn shots_restyled<'a>(
        &self,
        frame: i64,
        view: Viewpoint,
        restyled: impl Fn(ClipId) -> Option<&'a Appearance>,
    ) -> Vec<(usize, &Clip, Shot)> {
        let cameras = self.cameras_at(frame, view, &restyled);
        let mut shots: Vec<(usize, &Clip, Shot)> = self
            .visuals_at(frame)
            .map(|(layer, clip, visual)| {
                let visual = visual.with_appearance(restyled(clip.id()));
                let transforms = preset_transforms(clip, frame, &visual);
                let shot = self.shoot(layer, visual, transforms, &cameras, view);
                (layer, clip, shot)
            })
            .collect();
        ordered(&mut shots, |(_, _, shot)| Some(shot));
        shots
    }

    #[must_use]
    pub fn front_shot(&self, visual: Visual, view: Viewpoint) -> Shot {
        self.shoot(usize::MAX, visual, PresetTransforms::default(), &[], view)
    }
}
