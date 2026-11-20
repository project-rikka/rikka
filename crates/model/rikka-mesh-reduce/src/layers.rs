use glam::{Vec2, Vec4};
use rikka_mesh_edit::{Domain, EditMesh, LoopId};

use rikka_mesh_edit::{MathLayer, MathLayerKind};

#[derive(Clone, Copy, Debug)]
pub(crate) enum LayerValue {
    Vec2(Vec2),
    Color(Vec4),
}

impl LayerValue {
    fn scaled(self, factor: f32) -> LayerValue {
        match self {
            LayerValue::Vec2(value) => LayerValue::Vec2(value * factor),
            LayerValue::Color(value) => LayerValue::Color(value * factor),
        }
    }

    fn added(self, other: LayerValue) -> LayerValue {
        match (self, other) {
            (LayerValue::Vec2(a), LayerValue::Vec2(b)) => LayerValue::Vec2(a + b),
            (LayerValue::Color(a), LayerValue::Color(b)) => LayerValue::Color(a + b),
            (value, _) => value,
        }
    }
}

pub(crate) fn read(mesh: &EditMesh, layer: &MathLayer, lp: LoopId) -> Option<LayerValue> {
    match layer.kind {
        MathLayerKind::Vec2 => mesh
            .attribute::<Vec2>(&layer.name, Domain::Corner)
            .map(|values| LayerValue::Vec2(values[lp.index()])),
        MathLayerKind::Color => mesh
            .attribute::<Vec4>(&layer.name, Domain::Corner)
            .map(|values| LayerValue::Color(values[lp.index()])),
    }
}

pub(crate) fn write(mesh: &mut EditMesh, layer: &MathLayer, lp: LoopId, value: LayerValue) {
    match value {
        LayerValue::Vec2(value) => {
            if let Some(values) = mesh.attribute_mut::<Vec2>(&layer.name, Domain::Corner) {
                values[lp.index()] = value;
            }
        }
        LayerValue::Color(value) => {
            if let Some(values) = mesh.attribute_mut::<Vec4>(&layer.name, Domain::Corner) {
                values[lp.index()] = value;
            }
        }
    }
}

pub(crate) fn scale(mesh: &mut EditMesh, layer: &MathLayer, lp: LoopId, factor: f32) {
    if let Some(value) = read(mesh, layer, lp) {
        write(mesh, layer, lp, value.scaled(factor));
    }
}

pub(crate) fn add(mesh: &mut EditMesh, layer: &MathLayer, target: LoopId, source: LoopId) {
    if let (Some(a), Some(b)) = (read(mesh, layer, target), read(mesh, layer, source)) {
        write(mesh, layer, target, a.added(b));
    }
}

pub(crate) fn mix(
    mesh: &mut EditMesh,
    layer: &MathLayer,
    sources: [LoopId; 2],
    weights: [f32; 2],
    target: LoopId,
) {
    let (Some(a), Some(b)) = (read(mesh, layer, sources[0]), read(mesh, layer, sources[1])) else {
        return;
    };
    let mixed = match (a, b) {
        (LayerValue::Vec2(a), LayerValue::Vec2(b)) => {
            LayerValue::Vec2(a * weights[0] + b * weights[1])
        }
        (LayerValue::Color(a), LayerValue::Color(b)) => {
            LayerValue::Color(a * weights[0] + b * weights[1])
        }
        (value, _) => value,
    };
    write(mesh, layer, target, mixed);
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    min: LayerValue,
    max: LayerValue,
}

impl Bounds {
    pub(crate) fn new(kind: MathLayerKind) -> Bounds {
        match kind {
            MathLayerKind::Vec2 => Bounds {
                min: LayerValue::Vec2(Vec2::splat(f32::MAX)),
                max: LayerValue::Vec2(Vec2::splat(-f32::MAX)),
            },
            MathLayerKind::Color => Bounds {
                min: LayerValue::Color(Vec4::splat(f32::MAX)),
                max: LayerValue::Color(Vec4::splat(f32::MIN_POSITIVE)),
            },
        }
    }

    pub(crate) fn include(&mut self, value: LayerValue) {
        match (&mut self.min, &mut self.max, value) {
            (LayerValue::Vec2(min), LayerValue::Vec2(max), LayerValue::Vec2(value)) => {
                *min = min.min(value);
                *max = max.max(value);
            }
            (LayerValue::Color(min), LayerValue::Color(max), LayerValue::Color(value)) => {
                for axis in 0..4 {
                    if min[axis] > value[axis] {
                        min[axis] = value[axis];
                    }
                    if max[axis] < value[axis] {
                        max[axis] = value[axis];
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn center(&self) -> LayerValue {
        self.min.scaled(0.5).added(self.max.scaled(0.5))
    }
}
