use animate_look::world::EMITTER_POSITION;
use animate_look::{GraphRef, SocketKey, Target};
use animate_shader_editor::LookValues;
use animate_viewport::Marked;
use rikka_model::glam::Vec3;
use rikka_shader_graph::{NodeId, NodeKind, PortRef, Scope, SocketRef, SocketValue};

use crate::animate::Animate;

impl Animate {
    pub(crate) fn marked(&self) -> Option<Marked> {
        let (GraphRef::World, node) = self.shader.chosen_node(self.look())? else {
            return None;
        };
        let world = &self.look().world;
        let library = self.workbench.library();
        let placed = world.body().node(node).is_some_and(|held| {
            matches!(&held.kind, NodeKind::Extern(id) if library
                .get(id)
                .is_some_and(|spec| spec.inputs.iter().any(|port| port.name == EMITTER_POSITION)))
        });
        let driven = world
            .body()
            .source(&PortRef::new(node, EMITTER_POSITION))
            .is_some();
        Some(Marked {
            node,
            movable: placed && !driven,
        })
    }

    pub(crate) fn place(&mut self, node: NodeId, by: Vec3) {
        let target = Target::Socket(SocketKey::new(
            GraphRef::World,
            SocketRef::new(Scope::Main, node, EMITTER_POSITION),
        ));
        let frame = self.transport.frame() as f32;
        let Some(current) = self
            .look()
            .value_at(self.workbench.library(), &target, frame)
            .and_then(|value| value.three())
        else {
            return;
        };
        let moved = Vec3::from_array(current) + by;
        LookValues::set_value(
            self,
            target,
            Some(SocketValue::from(moved.to_array())),
            None,
        );
    }
}
