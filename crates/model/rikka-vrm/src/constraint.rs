use rikka_model_core::glam::{Quat, Vec3};

use rikka_model_core::Conversion;
use rikka_model_core::NodeId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RollAxis {
    #[default]
    X,
    Y,
    Z,
}

named!(RollAxis {
    X => "X",
    Y => "Y",
    Z => "Z",
});

impl RollAxis {
    #[must_use]
    pub fn direction(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    fn converted(self, conversion: &Conversion) -> Self {
        match nearest_axis(conversion.vector(self.direction())) {
            (0, _) => Self::X,
            (1, _) => Self::Y,
            _ => Self::Z,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AimAxis {
    #[default]
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

named!(AimAxis {
    PositiveX => "PositiveX",
    NegativeX => "NegativeX",
    PositiveY => "PositiveY",
    NegativeY => "NegativeY",
    PositiveZ => "PositiveZ",
    NegativeZ => "NegativeZ",
});

impl AimAxis {
    #[must_use]
    pub fn direction(self) -> Vec3 {
        match self {
            Self::PositiveX => Vec3::X,
            Self::NegativeX => Vec3::NEG_X,
            Self::PositiveY => Vec3::Y,
            Self::NegativeY => Vec3::NEG_Y,
            Self::PositiveZ => Vec3::Z,
            Self::NegativeZ => Vec3::NEG_Z,
        }
    }

    fn converted(self, conversion: &Conversion) -> Self {
        match nearest_axis(conversion.vector(self.direction())) {
            (0, true) => Self::PositiveX,
            (0, false) => Self::NegativeX,
            (1, true) => Self::PositiveY,
            (1, false) => Self::NegativeY,
            (2, true) => Self::PositiveZ,
            (_, _) => Self::NegativeZ,
        }
    }
}

fn nearest_axis(direction: Vec3) -> (usize, bool) {
    let axis = direction
        .abs()
        .to_array()
        .iter()
        .enumerate()
        .max_by(|(_, one), (_, other)| one.total_cmp(other))
        .map_or(0, |(axis, _)| axis);
    (axis, direction[axis] >= 0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    Roll {
        source: NodeId,
        axis: RollAxis,
        weight: f32,
    },
    Aim {
        source: NodeId,
        axis: AimAxis,
        weight: f32,
    },
    Rotation {
        source: NodeId,
        weight: f32,
    },
}

impl Constraint {
    #[must_use]
    pub fn source(&self) -> NodeId {
        match self {
            Self::Roll { source, .. }
            | Self::Aim { source, .. }
            | Self::Rotation { source, .. } => *source,
        }
    }

    #[must_use]
    pub fn weight(&self) -> f32 {
        match self {
            Self::Roll { weight, .. }
            | Self::Aim { weight, .. }
            | Self::Rotation { weight, .. } => *weight,
        }
    }

    pub(crate) fn convert(&mut self, conversion: &Conversion) {
        match self {
            Self::Roll { axis, .. } => *axis = axis.converted(conversion),
            Self::Aim { axis, .. } => *axis = axis.converted(conversion),
            Self::Rotation { .. } => {}
        }
    }

    #[must_use]
    pub fn resolve(&self, state: &ConstraintState) -> Quat {
        let weight = self.weight().clamp(0.0, 1.0);
        let target = match self {
            Self::Rotation { .. } => {
                state.destination_rest * (state.source_rest.inverse() * state.source)
            }
            Self::Roll { axis, .. } => {
                let delta = state.source_rest.inverse() * state.source;
                let in_parent = state.source_rest * delta * state.source_rest.inverse();
                let in_destination =
                    state.destination_rest.inverse() * in_parent * state.destination_rest;
                let roll = axis.direction();
                let turned = in_destination * roll;
                let from_to = Quat::from_rotation_arc(roll, turned.normalize_or(roll));
                state.destination_rest * from_to.inverse() * in_destination
            }
            Self::Aim { axis, .. } => {
                let from = (state.parent_world * state.destination_rest) * axis.direction();
                let to = (state.source_world_position - state.destination_world_position)
                    .normalize_or(from);
                let from_to = Quat::from_rotation_arc(from.normalize_or(Vec3::Z), to);
                state.parent_world.inverse() * from_to * state.parent_world * state.destination_rest
            }
        };
        state.destination_rest.slerp(target.normalize(), weight)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConstraintState {
    pub source: Quat,
    pub source_rest: Quat,
    pub destination_rest: Quat,
    pub parent_world: Quat,
    pub source_world_position: Vec3,
    pub destination_world_position: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeConstraint {
    pub node: NodeId,
    pub constraint: Constraint,
}
