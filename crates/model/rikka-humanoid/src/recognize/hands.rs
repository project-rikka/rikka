use rikka_model_core::glam::Vec3;
use rikka_model_core::{HumanBone, NodeId};

use super::Recognition;
use crate::frame::Frame;

const FINGERED: usize = 3;
const REACHING: f32 = 0.1;
const LONGEST_DIGIT: usize = 4;

pub(crate) fn recognize(frame: &Frame, uppers: &[NodeId]) -> Option<[NodeId; 2]> {
    let free = |node: NodeId| !uppers.iter().any(|upper| frame.contains(*upper, node));
    let joints: Vec<NodeId> = frame
        .weighted()
        .filter(|node| frame.is_joint(*node) && free(*node))
        .collect();
    let fingered: Vec<NodeId> = joints
        .iter()
        .copied()
        .filter(|node| digits(frame, *node).len() >= FINGERED)
        .collect();
    let side = |node: NodeId| frame.origin(node).x - frame.centre.x;
    let outermost = |candidates: &[NodeId], sign: f32| {
        candidates
            .iter()
            .copied()
            .filter(|node| side(*node) * sign > REACHING * frame.span)
            .max_by(|a, b| (side(*a) * sign).total_cmp(&(side(*b) * sign)))
    };
    let pair =
        |candidates: &[NodeId]| Some([outermost(candidates, 1.0)?, outermost(candidates, -1.0)?]);
    pair(&fingered).or_else(|| pair(&joints))
}

pub(crate) fn fingers(placing: &mut Recognition<'_>, hand: NodeId) {
    use HumanBone as B;
    let frame = placing.frame;
    let mut chains: Vec<Vec<NodeId>> = digits(frame, hand)
        .into_iter()
        .map(|root| digit(frame, root))
        .filter(|chain| !chain.is_empty())
        .collect();
    let Some(thumb) = thumb(frame, hand, &chains) else {
        return;
    };
    let thumb = chains.remove(thumb);
    let ahead = |chain: &Vec<NodeId>| frame.origin(chain[0]).dot(placing.facing.forward);
    chains.sort_by(|a, b| ahead(b).total_cmp(&ahead(a)));
    let left = placing.lateral(hand) >= 0.0;
    let sided = |bones: [HumanBone; 3], mirrored: [HumanBone; 3]| match left {
        true => bones,
        false => mirrored,
    };
    let thumb_bones = sided(
        [
            B::LeftThumbMetacarpal,
            B::LeftThumbProximal,
            B::LeftThumbDistal,
        ],
        [
            B::RightThumbMetacarpal,
            B::RightThumbProximal,
            B::RightThumbDistal,
        ],
    );
    let fingers = [
        sided(
            [
                B::LeftIndexProximal,
                B::LeftIndexIntermediate,
                B::LeftIndexDistal,
            ],
            [
                B::RightIndexProximal,
                B::RightIndexIntermediate,
                B::RightIndexDistal,
            ],
        ),
        sided(
            [
                B::LeftMiddleProximal,
                B::LeftMiddleIntermediate,
                B::LeftMiddleDistal,
            ],
            [
                B::RightMiddleProximal,
                B::RightMiddleIntermediate,
                B::RightMiddleDistal,
            ],
        ),
        sided(
            [
                B::LeftRingProximal,
                B::LeftRingIntermediate,
                B::LeftRingDistal,
            ],
            [
                B::RightRingProximal,
                B::RightRingIntermediate,
                B::RightRingDistal,
            ],
        ),
        sided(
            [
                B::LeftLittleProximal,
                B::LeftLittleIntermediate,
                B::LeftLittleDistal,
            ],
            [
                B::RightLittleProximal,
                B::RightLittleIntermediate,
                B::RightLittleDistal,
            ],
        ),
    ];
    let order: &[usize] = match chains.len() {
        0 => &[],
        1 => &[0],
        2 => &[0, 3],
        3 => &[0, 1, 3],
        _ => &[0, 1, 2, 3],
    };
    let mut placed = vec![(thumb_bones, thumb)];
    placed.extend(
        order
            .iter()
            .zip(chains)
            .map(|(finger, chain)| (fingers[*finger], chain)),
    );
    for (bones, chain) in placed {
        for (bone, node) in bones.into_iter().rev().zip(chain.into_iter().rev()) {
            placing.set(bone, Some(node));
        }
    }
}

fn thumb(frame: &Frame, hand: NodeId, chains: &[Vec<NodeId>]) -> Option<usize> {
    let at = frame.origin(hand);
    let ways: Vec<Vec3> = chains
        .iter()
        .map(|chain| (frame.origin(*chain.last().expect("a joint")) - at).normalize_or_zero())
        .collect();
    let isolation = |index: usize| {
        ways.iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, way)| ways[index].angle_between(*way))
            .fold(f32::MAX, f32::min)
    };
    let nearness = |index: usize| at.distance(frame.origin(chains[index][0]));
    match chains.len() {
        0 => None,
        1 | 2 => (0..chains.len()).min_by(|a, b| nearness(*a).total_cmp(&nearness(*b))),
        _ => (0..chains.len()).max_by(|a, b| isolation(*a).total_cmp(&isolation(*b))),
    }
}

fn digits(frame: &Frame, hand: NodeId) -> Vec<NodeId> {
    frame
        .children(hand)
        .iter()
        .copied()
        .filter(|child| !frame.children(*child).is_empty())
        .collect()
}

fn digit(frame: &Frame, root: NodeId) -> Vec<NodeId> {
    let mut joints = Vec::new();
    let mut node = Some(root);
    while let Some(current) = node
        && joints.len() < LONGEST_DIGIT
    {
        if frame.is_joint(current) {
            joints.push(current);
        }
        node = frame
            .children(current)
            .iter()
            .copied()
            .max_by(|a, b| frame.subtree_flesh(*a).total_cmp(&frame.subtree_flesh(*b)));
    }
    joints
}
