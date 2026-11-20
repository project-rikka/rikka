use crate::document::Node;
use crate::read::props::Props;
use rikka_model_core::{Axis, CoordinateSystem};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub coordinates: CoordinateSystem,
    pub unit_scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            coordinates: CoordinateSystem::CANONICAL,
            unit_scale: 0.01,
        }
    }
}

impl Settings {
    pub(crate) fn read(nodes: &[Node]) -> Self {
        let Some(props) = nodes
            .iter()
            .find(|node| node.name == "GlobalSettings")
            .map(Props::read)
        else {
            return Self::default();
        };

        let axis = |name: &str, sign: &str, fallback: Axis| match (
            props.integer(name),
            props.integer(sign).unwrap_or(1),
        ) {
            (Some(axis), sign) => directed(axis, sign).unwrap_or(fallback),
            (None, _) => fallback,
        };
        let right = axis("CoordAxis", "CoordAxisSign", Axis::X);
        let up = axis("UpAxis", "UpAxisSign", Axis::Y);
        let front = axis("FrontAxis", "FrontAxisSign", Axis::Z);

        let coordinates = CoordinateSystem {
            right,
            up,
            forward: front.opposite(),
        };
        Self {
            coordinates: match distinct(coordinates) {
                true => coordinates,
                false => CoordinateSystem::CANONICAL,
            },
            unit_scale: props.real("UnitScaleFactor", 1.0) * 0.01,
        }
    }
}

fn directed(axis: i64, sign: i64) -> Option<Axis> {
    let positive = sign >= 0;
    Some(match (axis, positive) {
        (0, true) => Axis::X,
        (0, false) => Axis::NegX,
        (1, true) => Axis::Y,
        (1, false) => Axis::NegY,
        (2, true) => Axis::Z,
        (2, false) => Axis::NegZ,
        _ => return None,
    })
}

fn distinct(system: CoordinateSystem) -> bool {
    let named = |axis: Axis| match axis {
        Axis::X | Axis::NegX => 0,
        Axis::Y | Axis::NegY => 1,
        Axis::Z | Axis::NegZ => 2,
    };
    let axes = [named(system.right), named(system.up), named(system.forward)];
    axes[0] != axes[1] && axes[1] != axes[2] && axes[0] != axes[2]
}
