use super::shot::{Shot, Viewpoint, ordered, preset_transforms};
use crate::clip::{Clip, ClipId};
use crate::project::Project;
use crate::transitions::TransitionSpan;
use crate::visual::Appearance;

#[derive(Clone, Debug)]
pub enum SceneItem<'a> {
    Clip {
        layer: usize,
        clip: &'a Clip,
        shot: Box<Shot>,
    },
    TransitionSpan {
        layer: usize,
        shots: Box<TransitionShots<'a>>,
    },
}

#[derive(Clone, Debug)]
pub struct TransitionShots<'a> {
    pub transition_span: TransitionSpan<'a>,
    pub out: Option<Shot>,
    pub into: Option<Shot>,
}

impl Project {
    #[must_use]
    pub fn scene_restyled<'a>(
        &self,
        frame: i64,
        view: Viewpoint,
        restyled: impl Fn(ClipId) -> Option<&'a Appearance>,
    ) -> Vec<SceneItem<'_>> {
        let cameras = self.cameras_at(frame, view, &restyled);
        let shot_of = |layer: usize, clip: &Clip| {
            let visual = self
                .visual(clip, frame)?
                .with_appearance(restyled(clip.id()));
            let transforms = preset_transforms(clip, frame, &visual);
            Some(self.shoot(layer, visual, transforms, &cameras, view))
        };
        let mut scene = Vec::new();
        for (layer, layer_ref) in self.layers().iter().enumerate() {
            if !layer_ref.is_shown() {
                continue;
            }
            if let Some(transition_span) = self.transition_span_at(layer, frame) {
                let out = transition_span.out.and_then(|clip| shot_of(layer, clip));
                let into = transition_span.into.and_then(|clip| shot_of(layer, clip));
                scene.push(SceneItem::TransitionSpan {
                    layer,
                    shots: Box::new(TransitionShots {
                        transition_span,
                        out,
                        into,
                    }),
                });
                continue;
            }
            let Some(clip) = layer_ref.clip_at(frame) else {
                continue;
            };
            if let Some(shot) = shot_of(layer, clip) {
                scene.push(SceneItem::Clip {
                    layer,
                    clip,
                    shot: Box::new(shot),
                });
            }
        }
        ordered(&mut scene, |showing| match showing {
            SceneItem::Clip { shot, .. } => Some(&**shot),
            SceneItem::TransitionSpan { .. } => None,
        });
        scene
    }
}
