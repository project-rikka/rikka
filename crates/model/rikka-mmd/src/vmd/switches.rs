use rikka_model_core::Model;

use super::super::pmx::Pmx;
use super::super::runtime::Puppet;
use super::document::{FRAMES_PER_SECOND, Vmd};

const SETTLED: f32 = 1e-3;

#[derive(Clone, Debug, Default)]
pub struct VmdSwitches {
    frames: Vec<Switched>,
}

#[derive(Clone, Debug)]
struct Switched {
    frame: u32,
    visible: bool,
    reaching: Vec<(usize, bool)>,
}

impl Vmd {
    #[must_use]
    pub fn switches(&self, model: &Model) -> VmdSwitches {
        let Some(pmx) = model.extensions.get::<Pmx>() else {
            return VmdSwitches::default();
        };
        let mut frames: Vec<Switched> = self
            .properties
            .iter()
            .map(|frame| Switched {
                frame: frame.frame,
                visible: frame.visible,
                reaching: frame
                    .ik
                    .iter()
                    .filter_map(|(name, reaching)| {
                        let bone = pmx
                            .bones
                            .iter()
                            .position(|bone| bone.name == *name && bone.ik.is_some())?;
                        Some((bone, *reaching))
                    })
                    .collect(),
            })
            .collect();
        frames.sort_by_key(|frame| frame.frame);
        frames.dedup_by(|later, earlier| {
            let same = later.frame == earlier.frame;
            if same {
                std::mem::swap(later, earlier);
            }
            same
        });
        VmdSwitches { frames }
    }
}

impl VmdSwitches {
    #[must_use]
    pub fn visible(&self, time: f32) -> bool {
        self.at(time).is_none_or(|state| state.visible)
    }

    pub fn apply(&self, time: f32, puppet: &mut Puppet) {
        puppet.reach_everywhere();
        let Some(state) = self.at(time) else {
            return;
        };
        for (bone, reaching) in &state.reaching {
            puppet.set_reaching(*bone, *reaching);
        }
    }

    fn at(&self, time: f32) -> Option<&Switched> {
        let frame = (time * FRAMES_PER_SECOND + SETTLED).floor().max(0.0) as u32;
        let next = self
            .frames
            .partition_point(|keyframe| keyframe.frame <= frame);
        match next.checked_sub(1) {
            Some(previous) => self.frames.get(previous),
            None => self.frames.first(),
        }
    }
}
