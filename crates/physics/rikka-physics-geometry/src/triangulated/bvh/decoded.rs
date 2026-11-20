use std::fmt;
use std::sync::Arc;

use rikka_physics_math::{Aabb, Vec3, Vector};

use super::bounds4::Bounds4;
use super::node::{Child, EMPTY_CHILD, QuantizedNode4, decode_child};
use super::quantized::decode;
use super::traverse::{self, Nodes};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DecodedNode {
    min: [[f32; 4]; 3],
    max: [[f32; 4]; 3],
    children: [u32; 4],
}

#[derive(Clone, Default, PartialEq)]
pub(crate) struct DecodedBounds(Arc<[DecodedNode]>);

impl fmt::Debug for DecodedBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodedBounds")
            .field("nodes", &self.0.len())
            .finish()
    }
}

impl DecodedBounds {
    pub(crate) fn decode(nodes: &[QuantizedNode4], bounds: Aabb) -> Self {
        let mut frames = vec![bounds; nodes.len()];
        let decoded = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let children = decode(node, &frames[index]);
                for slot in 0..4 {
                    if let Child::Node(child) = node.child(slot) {
                        frames[child as usize] = children.lane(slot);
                    }
                }
                DecodedNode {
                    min: children.min.map(|lanes| lanes.to_array()),
                    max: children.max.map(|lanes| lanes.to_array()),
                    children: node.children,
                }
            })
            .collect();
        Self(decoded)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DecodedTree<'a> {
    nodes: &'a [DecodedNode],
}

impl<'a> DecodedTree<'a> {
    #[inline]
    pub(crate) fn new(bounds: &'a DecodedBounds) -> Self {
        Self { nodes: &bounds.0 }
    }

    #[inline]
    pub(crate) fn query_leaves(&self, bounds: Aabb, visit: impl FnMut(u32, u32) -> bool) -> bool {
        traverse::query_leaves(self, &bounds, visit)
    }

    #[inline]
    pub(crate) fn box_cast(
        &self,
        bounds: Aabb,
        translation: Vec3,
        max_fraction: f32,
        visit: impl FnMut(u32, f32) -> f32,
    ) -> f32 {
        traverse::sweep(
            self,
            bounds.center(),
            translation,
            bounds.extent(),
            max_fraction,
            visit,
        )
    }
}

impl Nodes for DecodedTree<'_> {
    type Frame = ();

    #[inline]
    fn root(&self) -> Option<(u32, ())> {
        (!self.nodes.is_empty()).then_some((0, ()))
    }

    #[inline]
    fn load(&self, node: u32, _: &()) -> Bounds4 {
        let node = &self.nodes[node as usize];
        Bounds4::from_f32(
            [&node.min[0], &node.min[1], &node.min[2]],
            [&node.max[0], &node.max[1], &node.max[2]],
        )
    }

    #[inline]
    fn child(&self, node: u32, slot: usize) -> Child {
        decode_child(self.nodes[node as usize].children[slot])
    }

    #[inline]
    fn occupied(&self, node: u32) -> u32 {
        let children = &self.nodes[node as usize].children;
        (0..4).fold(0, |mask, slot| {
            mask | (u32::from(children[slot] != EMPTY_CHILD) << slot)
        })
    }

    #[inline]
    fn child_frame(_: &Bounds4, _: usize) {}
}
