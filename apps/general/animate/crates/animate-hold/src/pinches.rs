use animate_body::{Body, Side};
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Instance, Model, NodeId};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pinched {
    node: NodeId,
    offset: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pinches {
    hands: [Option<Pinched>; 2],
}

impl Pinches {
    #[must_use]
    pub fn new(body: Option<&Body>, model: &Model) -> Self {
        let Some(body) = body else {
            return Self::default();
        };
        let rest = model.world_transforms();
        let hand = |side: Side| {
            let pinch = body.pinch(side)?;
            let node = body.mapping().node(side.hand())?;
            let world = rest.get(node.index())?;
            Some(Pinched {
                node,
                offset: world.inverse().transform_point3(pinch.rest),
            })
        };
        Self {
            hands: Side::BOTH.map(hand),
        }
    }

    #[must_use]
    pub fn point(&self, side: Side, instance: &Instance) -> Option<Vec3> {
        let target = self.hands[slot(side)]?;
        let world = instance.transform() * *instance.worlds().get(target.node.index())?;
        Some(world.transform_point3(target.offset))
    }
}

fn slot(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Right => 1,
    }
}
