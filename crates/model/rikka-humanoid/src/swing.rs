use rikka_model_core::glam::Vec3;
use rikka_model_core::{HumanBone, NodeId};

use crate::facing::Facing;
use crate::frame::Frame;

const MIRRORED: f32 = 0.02;
const APART: f32 = 0.01;
const COMPACT: f32 = 0.08;
const HANGING: f32 = 0.5;
const TORSO: [HumanBone; 3] = [HumanBone::Spine, HumanBone::Chest, HumanBone::UpperChest];

#[derive(Clone, Debug, PartialEq)]
pub struct Swing {
    root: NodeId,
    nodes: Vec<NodeId>,
    anchor: Option<NodeId>,
    bone: Option<HumanBone>,
    flesh: bool,
}

impl Swing {
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.root
    }

    #[must_use]
    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    #[must_use]
    pub fn anchor(&self) -> Option<NodeId> {
        self.anchor
    }

    #[must_use]
    pub fn bone(&self) -> Option<HumanBone> {
        self.bone
    }

    #[must_use]
    pub fn is_flesh(&self) -> bool {
        self.flesh
    }
}

pub(crate) fn swings(
    frame: &Frame,
    regions: &[Option<HumanBone>],
    facing: &Facing,
) -> (Vec<Swing>, Vec<Option<usize>>) {
    let mut hung: Vec<Option<usize>> = vec![None; frame.count()];
    let mut swings: Vec<Swing> = Vec::new();
    for node in &frame.order {
        let parent = frame.parents[node.index()];
        let above = parent.and_then(|parent| hung[parent.index()]);
        if !frame.swinging[node.index()] {
            hung[node.index()] = above;
            continue;
        }
        let joined = parent
            .filter(|parent| frame.swinging[parent.index()])
            .and(above);
        match joined {
            Some(swing) => {
                hung[node.index()] = Some(swing);
                swings[swing].nodes.push(*node);
            }
            None => {
                hung[node.index()] = Some(swings.len());
                swings.push(Swing {
                    root: *node,
                    nodes: vec![*node],
                    anchor: parent,
                    bone: parent.and_then(|parent| regions[parent.index()]),
                    flesh: false,
                });
            }
        }
    }
    for swing in &mut swings {
        swing.nodes.sort_unstable();
    }
    let roots: Vec<Vec3> = swings
        .iter()
        .map(|swing| facing.place(frame.origin(swing.root)))
        .collect();
    let near = MIRRORED * frame.span;
    let fleshy: Vec<bool> = (0..swings.len())
        .map(|index| {
            let swing = &swings[index];
            let root = roots[index];
            swing.bone.is_some_and(|bone| TORSO.contains(&bone))
                && root.x.abs() >= APART * frame.span
                && (0..swings.len()).any(|other| {
                    let mirror = roots[other];
                    other != index
                        && swings[other].bone == swing.bone
                        && (root.x + mirror.x).abs() <= near
                        && (root.y - mirror.y).abs() <= near
                        && (root.z - mirror.z).abs() <= near
                })
                && bulging(frame, swing, facing.forward)
        })
        .collect();
    for (swing, flesh) in swings.iter_mut().zip(fleshy) {
        swing.flesh = flesh;
    }
    (swings, hung)
}

fn bulging(frame: &Frame, swing: &Swing, forward: Vec3) -> bool {
    let root = frame.origin(swing.root);
    let Some(anchor) = swing.anchor.map(|anchor| frame.origin(anchor)) else {
        return false;
    };
    let tip = swing
        .nodes
        .iter()
        .map(|node| frame.origin(*node))
        .max_by(|a, b| a.distance(root).total_cmp(&b.distance(root)))
        .unwrap_or(root);
    let along = tip - root;
    let compact = along.length() <= COMPACT * frame.span;
    let hanging = -along.y > HANGING * along.length();
    (root - anchor).dot(forward) > 0.0 && compact && !hanging
}
