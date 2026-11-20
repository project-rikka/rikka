use rikka_model_core::{HumanBone, Humanoid, NodeId};

use crate::frame::Frame;

pub(crate) fn parts(frame: &Frame, humanoid: &Humanoid) -> Vec<Option<HumanBone>> {
    let count = frame.count();
    let mut parts: Vec<Option<HumanBone>> = vec![None; count];
    for (bone, node) in humanoid.bones() {
        parts[node.index()] = Some(bone);
    }
    for index in 0..count {
        let alias = frame.alias(NodeId(index as u32));
        if parts[index].is_none() && alias.index() != index {
            parts[index] = parts[alias.index()];
        }
    }
    let assigned: Vec<NodeId> = (0..count)
        .filter(|index| parts[*index].is_some())
        .map(|index| NodeId(index as u32))
        .collect();
    for node in assigned {
        let mut between = Vec::new();
        let mut cursor = frame.parents[node.index()];
        while let Some(current) = cursor {
            if let Some(upper) = parts[current.index()] {
                for node in between {
                    parts[node] = Some(upper);
                }
                break;
            }
            if between.len() > count {
                break;
            }
            between.push(current.index());
            cursor = frame.parents[current.index()];
        }
    }
    if let Some(hips) = humanoid.bone(HumanBone::Hips) {
        let mut cursor = frame.parents[hips.index()];
        while let Some(current) = cursor.filter(|current| parts[current.index()].is_none()) {
            parts[current.index()] = Some(HumanBone::Hips);
            cursor = frame.parents[current.index()];
        }
    }
    for node in &frame.order {
        if frame.is_follower(*node)
            && parts[node.index()].is_none()
            && let Some(part) = frame.parents[node.index()].and_then(|parent| parts[parent.index()])
        {
            parts[node.index()] = Some(part);
        }
    }
    parts
}

pub(crate) fn regions(frame: &Frame, parts: &[Option<HumanBone>]) -> Vec<Option<HumanBone>> {
    let mut regions = parts.to_vec();
    for node in &frame.order {
        if regions[node.index()].is_none()
            && let Some(parent) = frame.parents[node.index()]
        {
            regions[node.index()] = regions[parent.index()];
        }
    }
    regions
}
