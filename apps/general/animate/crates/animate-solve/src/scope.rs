use std::ops::Range;

use animate_body::{Body, Dof, Side, TORSO, muscles, muscles_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    Torso,
    Shoulder(Side),
}

impl Region {
    fn muscles(self) -> Range<usize> {
        match self {
            Self::Torso => TORSO,
            Self::Shoulder(side) => muscles_of(side.shoulder()),
        }
    }
}

pub(crate) fn bending(body: &Body, regions: &[Region]) -> Vec<usize> {
    regions
        .iter()
        .flat_map(|region| region.muscles())
        .filter(|index| {
            let muscle = muscles()[*index];
            muscle.dof != Dof::Twist && body.has(muscle.bone)
        })
        .collect()
}
