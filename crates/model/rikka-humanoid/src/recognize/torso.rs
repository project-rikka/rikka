use rikka_model_core::{Follow, NodeId};

use crate::frame::Frame;

const CENTRAL: f32 = 0.04;
const LEVEL: f32 = 0.006;
const EYES_APART: (f32, f32) = (0.01, 0.07);
const SAME_RATIO: f32 = 1e-3;

#[derive(Clone, Debug)]
pub(crate) struct Torso {
    pub(crate) hips: NodeId,
    pub(crate) spine: Vec<NodeId>,
    pub(crate) chest: Option<NodeId>,
    pub(crate) neck: Option<NodeId>,
    pub(crate) head: NodeId,
    pub(crate) eyes: Option<[NodeId; 2]>,
}

pub(crate) fn recognize(
    frame: &Frame,
    uppers: &[NodeId],
    hands: Option<&[NodeId; 2]>,
) -> Option<Torso> {
    let legs = match uppers {
        [left, right] => frame.common(*left, *right),
        _ => None,
    };
    let chest = hands.and_then(|[left, right]| frame.common(*left, *right));
    let hips = match (legs, chest) {
        (Some(legs), Some(chest)) => frame.common(legs, chest)?,
        (Some(legs), None) => legs,
        (None, Some(chest)) => chest,
        (None, None) => return None,
    };
    let blocked: Vec<NodeId> = uppers
        .iter()
        .chain(hands.into_iter().flatten())
        .copied()
        .collect();
    let start = chest.unwrap_or(hips);
    let trunk: Vec<NodeId> = std::iter::once(hips)
        .chain(frame.path(hips, start))
        .collect();
    let top = |node: NodeId| {
        frame
            .subtree(node)
            .iter()
            .map(|descendant| frame.origin(*descendant).y)
            .fold(f32::MIN, f32::max)
    };
    let crown = trunk
        .iter()
        .flat_map(|node| frame.children(*node).iter().copied())
        .filter(|child| {
            !trunk.contains(child)
                && !blocked
                    .iter()
                    .any(|blocker| frame.contains(*child, *blocker))
        })
        .map(|child| (child, top(child)))
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(child, _)| child)?;
    let midline = frame.origin(hips).x;
    let head = frame
        .subtree(crown)
        .into_iter()
        .filter(|node| {
            frame.is_joint(*node) && (frame.origin(*node).x - midline).abs() <= CENTRAL * frame.span
        })
        .max_by(|a, b| frame.flesh(*a).total_cmp(&frame.flesh(*b)))?;
    let mut spine = frame.joints(&frame.path(hips, head));
    if spine.last() == Some(&head) {
        spine.pop();
    }
    let neck = match chest {
        Some(chest) => spine
            .last()
            .copied()
            .filter(|last| !frame.contains(*last, chest)),
        None => spine.last().copied().filter(|_| spine.len() >= 2),
    };
    if neck.is_some() {
        spine.pop();
    }
    Some(Torso {
        hips,
        spine,
        chest,
        neck,
        head,
        eyes: eyes(frame, head),
    })
}

fn eyes(frame: &Frame, head: NodeId) -> Option<[NodeId; 2]> {
    let base = frame.origin(head).y;
    let candidates: Vec<NodeId> = frame
        .subtree(head)
        .into_iter()
        .filter(|node| *node != head && frame.origin(*node).y > base)
        .collect();
    conjugate(frame, head, &candidates).or_else(|| {
        let fleshy = fleshiest(frame, head, &candidates)?;
        let [a, b] = fleshy;
        let lifted = [pivot(frame, head, a, b), pivot(frame, head, b, a)];
        Some(match is_pair(frame, head, lifted) {
            true => lifted,
            false => fleshy,
        })
    })
}

fn conjugate(frame: &Frame, head: NodeId, candidates: &[NodeId]) -> Option<[NodeId; 2]> {
    let led: Vec<Follow> = candidates
        .iter()
        .filter_map(|node| frame.follow(*node))
        .filter(|follow| follow.ratio > 0.0)
        .collect();
    best_pair(&led, |a, b| {
        let pair = [a.node, b.node];
        let together = a.source == b.source
            && (a.ratio - b.ratio).abs() <= SAME_RATIO
            && is_pair(frame, head, pair);
        together.then(|| {
            (
                pair,
                frame.subtree_flesh(a.node) + frame.subtree_flesh(b.node),
            )
        })
    })
}

fn fleshiest(frame: &Frame, head: NodeId, candidates: &[NodeId]) -> Option<[NodeId; 2]> {
    let fleshy: Vec<NodeId> = candidates
        .iter()
        .copied()
        .filter(|node| frame.flesh(*node) > 0.0)
        .collect();
    best_pair(&fleshy, |a, b| {
        is_pair(frame, head, [*a, *b]).then(|| ([*a, *b], frame.flesh(*a) + frame.flesh(*b)))
    })
}

fn best_pair<T>(
    items: &[T],
    score: impl Fn(&T, &T) -> Option<([NodeId; 2], f32)>,
) -> Option<[NodeId; 2]> {
    let mut best: Option<([NodeId; 2], f32)> = None;
    for (index, a) in items.iter().enumerate() {
        for b in &items[index + 1..] {
            if let Some((pair, value)) = score(a, b)
                && best.is_none_or(|(_, best_value)| value > best_value)
            {
                best = Some((pair, value));
            }
        }
    }
    best.map(|(pair, _)| pair)
}

fn pivot(frame: &Frame, head: NodeId, eye: NodeId, other: NodeId) -> NodeId {
    let mut pivot = eye;
    while let Some(parent) = frame.above(pivot) {
        if parent == head || frame.flesh(parent) > 0.0 || frame.contains(parent, other) {
            break;
        }
        pivot = parent;
    }
    pivot
}

fn is_pair(frame: &Frame, head: NodeId, [a, b]: [NodeId; 2]) -> bool {
    let span = frame.span;
    let (pa, pb) = (frame.origin(a), frame.origin(b));
    let apart = (pa.x - pb.x).abs();
    (pa.y - pb.y).abs() <= LEVEL * span
        && (pa.z - pb.z).abs() <= LEVEL * span
        && ((pa.x + pb.x) * 0.5 - frame.origin(head).x).abs() <= LEVEL * span
        && apart >= EYES_APART.0 * span
        && apart <= EYES_APART.1 * span
}
