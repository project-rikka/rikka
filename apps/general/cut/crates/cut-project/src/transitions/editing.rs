use std::sync::Arc;

use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::Project;

use super::edit_point::EditPoint;
use super::ends::Ends;
use super::transition::Transition;

impl Project {
    pub(crate) fn transit(
        &mut self,
        edit_points: &[EditPoint],
        transition: Option<&Transition>,
    ) -> Result<Applied> {
        if edit_points.is_empty() {
            return Err(Error::NoEditPoint);
        }
        let canonical: Vec<EditPoint> = edit_points
            .iter()
            .map(|edit_point| {
                self.canonical_edit_point(*edit_point)
                    .ok_or(Error::UnknownClip)
            })
            .collect::<Result<_>>()?;
        let transition = transition.map(|transition| transition.checked());
        for edit_point in canonical {
            let clip = self.visual_clip_mut(edit_point.clip)?;
            clip.transitions.set(edit_point.edge, transition);
        }
        Ok(Applied::default())
    }

    pub(crate) fn normalize_transitions(&mut self) {
        for index in 0..self.layers.len() {
            let clips = &self.layers[index].clips;
            let visual: Vec<bool> = clips.iter().map(|clip| self.is_visual(clip)).collect();
            let joined: Vec<bool> = clips
                .windows(2)
                .map(|pair| self.joins(&pair[0], &pair[1]))
                .collect();
            let cleared = clips
                .iter()
                .zip(&visual)
                .any(|(clip, visual)| !visual && clip.transitions != Ends::default());
            let handed = joined
                .iter()
                .enumerate()
                .any(|(at, joined)| *joined && visual[at] && clips[at].transitions.out.is_some());
            if !cleared && !handed {
                continue;
            }
            let clips = &mut Arc::make_mut(&mut self.layers[index]).clips;
            for (clip, visual) in clips.iter_mut().zip(&visual) {
                if !visual {
                    clip.transitions = Ends::default();
                }
            }
            for (at, joined) in joined.into_iter().enumerate() {
                if joined
                    && let Some(out) = clips[at].transitions.out.take()
                    && clips[at + 1].transitions.into.is_none()
                {
                    clips[at + 1].transitions.into = Some(out);
                }
            }
        }
    }
}
