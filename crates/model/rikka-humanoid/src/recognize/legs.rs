use rikka_model_core::NodeId;

use super::bend;
use crate::frame::Frame;
use rikka_model_core::RigFacts;

const BESIDE: f32 = 0.02;
const STEEP: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Leg {
    pub(crate) upper: NodeId,
    pub(crate) lower: Option<NodeId>,
    pub(crate) foot: NodeId,
    pub(crate) toes: Option<NodeId>,
}

pub(crate) fn recognize(frame: &Frame, evidence: &RigFacts) -> Option<[Leg; 2]> {
    reached(frame, evidence).or_else(|| hanging(frame))
}

fn reached(frame: &Frame, evidence: &RigFacts) -> Option<[Leg; 2]> {
    let mut legs: Vec<Leg> = evidence
        .reaches
        .iter()
        .filter_map(|reach| {
            let knee = frame.alias(*reach.links.first()?);
            let thigh = frame.alias(*reach.links.get(1)?);
            let ankle = frame.alias(reach.target);
            let height = |node: NodeId| frame.origin(node).y;
            let chained = [thigh, knee, ankle]
                .iter()
                .all(|node| frame.is_weighted(*node))
                && thigh != knee
                && knee != ankle
                && frame.contains(thigh, knee)
                && frame.contains(knee, ankle)
                && height(ankle) < height(knee)
                && height(knee) < height(thigh);
            chained.then(|| Leg {
                upper: thigh,
                lower: Some(knee),
                foot: ankle,
                toes: toes(frame, ankle),
            })
        })
        .collect();
    legs.sort_by(|a, b| frame.origin(a.foot).y.total_cmp(&frame.origin(b.foot).y));
    let side = |leg: &&Leg| frame.origin(leg.foot).x - frame.centre.x;
    let left = legs.iter().find(|leg| side(leg) > 0.0)?;
    let right = legs.iter().find(|leg| side(leg) < 0.0)?;
    Some([*left, *right])
}

fn hanging(frame: &Frame) -> Option<[Leg; 2]> {
    let lowest = |sign: f32| {
        frame
            .weighted()
            .filter(|node| {
                frame.is_joint(*node)
                    && (frame.origin(*node).x - frame.centre.x) * sign > BESIDE * frame.span
            })
            .min_by(|a, b| frame.origin(*a).y.total_cmp(&frame.origin(*b).y))
    };
    let (left, right) = (lowest(1.0)?, lowest(-1.0)?);
    let branch = frame.common(left, right)?;
    Some([limb(frame, branch, left)?, limb(frame, branch, right)?])
}

fn limb(frame: &Frame, branch: NodeId, lowest: NodeId) -> Option<Leg> {
    let chain = frame.joints(&frame.path(branch, lowest));
    let (rest, foot, toes) = match chain.as_slice() {
        [rest @ .., foot, toes] if is_toe(frame, *foot, *toes) => (rest, *foot, Some(*toes)),
        [rest @ .., foot] => (rest, *foot, None),
        [] => return None,
    };
    let (_, upper, lower) = bend(frame, rest, foot)?;
    Some(Leg {
        upper,
        lower,
        foot,
        toes,
    })
}

fn toes(frame: &Frame, foot: NodeId) -> Option<NodeId> {
    frame
        .children(foot)
        .iter()
        .copied()
        .filter(|child| frame.is_joint(*child))
        .max_by(|a, b| frame.subtree_flesh(*a).total_cmp(&frame.subtree_flesh(*b)))
        .filter(|toes| is_toe(frame, foot, *toes))
}

fn is_toe(frame: &Frame, foot: NodeId, toes: NodeId) -> bool {
    let along = frame.origin(toes) - frame.origin(foot);
    along.x.hypot(along.z) > STEEP * -along.y
}
