mod table;
mod words;

use std::collections::{HashMap, HashSet};

use rikka_model_core::{HumanBone, Humanoid, Model, NodeId};

use crate::frame::Frame;
use crate::lineage::Lineage;

use table::{
    BIPED_FINGER, DIGITS, Digit, FINGER_FILLERS, HEAD, JAW, LIMBS, Limb, METACARPALS, joint_rank,
};
use words::{Side, Words, is_number, words};

const JOINTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    priority: usize,
    unbound: bool,
    depth: u32,
    node: u32,
}

#[derive(Clone, Copy, Debug)]
struct Phalanx {
    node: NodeId,
    order: u32,
    metacarpal: bool,
}

fn lifted(frame: &Frame) -> Lineage {
    let count = frame.count();
    let flat = || Lineage::new(vec![None; count], vec![0; count]);
    let parents: Vec<Option<NodeId>> = (0..count)
        .map(|index| {
            let parent = frame.parents[index]?;
            let lifted = frame.alias(parent);
            Some(match lifted.index() == index {
                true => parent,
                false => lifted,
            })
        })
        .collect();
    let mut depth: Vec<Option<u32>> = vec![None; count];
    for index in 0..count {
        let mut climbed = Vec::new();
        let mut cursor = Some(NodeId(index as u32));
        let mut known = 0;
        while let Some(node) = cursor {
            if let Some(at) = depth[node.index()] {
                known = at + 1;
                break;
            }
            if climbed.len() > count {
                return flat();
            }
            climbed.push(node);
            cursor = parents[node.index()];
        }
        for node in climbed.into_iter().rev() {
            depth[node.index()] = Some(known);
            known += 1;
        }
    }
    Lineage::new(
        parents,
        depth.into_iter().map(|at| at.unwrap_or(0)).collect(),
    )
}

#[derive(Default)]
struct Candidates {
    head: Vec<(Rank, NodeId)>,
    jaw: Vec<(Rank, NodeId)>,
    limbs: HashMap<(Limb, Side), Vec<(Rank, NodeId)>>,
    digits: HashMap<(Digit, Side), Vec<Phalanx>>,
}

pub(crate) fn named(model: &Model, frame: &Frame) -> Option<Humanoid> {
    let lineage = lifted(frame);
    let leaf: Vec<bool> = model
        .nodes
        .iter()
        .map(|node| node.children.is_empty())
        .collect();
    let bound: HashSet<NodeId> = model
        .skins
        .iter()
        .flat_map(|skin| skin.joints.iter().copied())
        .collect();
    let mut candidates = Candidates::default();
    for (index, node) in model.nodes.iter().enumerate() {
        let id = NodeId(index as u32);
        let Some(words) = words(&node.name) else {
            continue;
        };
        let rank = |priority: usize| Rank {
            priority,
            unbound: !bound.contains(&id),
            depth: lineage.depth(id),
            node: id.0,
        };
        let key = words.core.concat();
        if let Some(side) = words.side {
            if let Some((digit, order, metacarpal)) = phalanx(&words) {
                candidates
                    .digits
                    .entry((digit, side))
                    .or_default()
                    .push(Phalanx {
                        node: id,
                        order,
                        metacarpal,
                    });
                continue;
            }
            for limb in LIMBS {
                if let Some(priority) = limb.keys().iter().position(|known| *known == key) {
                    candidates
                        .limbs
                        .entry((limb, side))
                        .or_default()
                        .push((rank(priority), id));
                }
            }
        } else if let Some(priority) = HEAD.iter().position(|known| *known == key) {
            candidates.head.push((rank(priority), id));
        } else if let Some(priority) = JAW.iter().position(|known| *known == key) {
            candidates.jaw.push((rank(priority), id));
        }
    }
    Placing::new(&lineage, &leaf, candidates).place()
}

fn phalanx(words: &Words) -> Option<(Digit, u32, bool)> {
    let core = &words.core;
    if let Some(at) = core
        .iter()
        .position(|word| Digit::from_word(word).is_some())
    {
        let digit = Digit::from_word(&core[at])?;
        let mut order = None;
        let mut metacarpal = false;
        for (index, word) in core.iter().enumerate() {
            if index == at || FINGER_FILLERS.contains(&word.as_str()) {
                continue;
            }
            if METACARPALS.contains(&word.as_str()) {
                metacarpal = true;
            }
            if let Some(rank) = joint_rank(word) {
                order.get_or_insert(rank);
            } else if is_number(word) {
                order.get_or_insert(word.parse().ok()?);
            } else if !METACARPALS.contains(&word.as_str()) {
                return None;
            }
        }
        return Some((digit, order.unwrap_or(0), metacarpal));
    }
    let at = core.iter().position(|word| word == BIPED_FINGER)?;
    let number = core.get(at + 1).filter(|word| is_number(word))?;
    if core.len() != at + 2 {
        return None;
    }
    let (first, rest) = number.split_at(1);
    let digit = Digit::from_biped(first.parse().ok()?)?;
    let order = match rest.is_empty() {
        true => 0,
        false => rest.parse().ok()?,
    };
    Some((digit, order, false))
}

struct Placing<'t> {
    tree: &'t Lineage,
    leaf: &'t [bool],
    candidates: Candidates,
    humanoid: Humanoid,
    used: HashSet<NodeId>,
}

impl<'t> Placing<'t> {
    fn new(tree: &'t Lineage, leaf: &'t [bool], candidates: Candidates) -> Self {
        Self {
            tree,
            leaf,
            candidates,
            humanoid: Humanoid::default(),
            used: HashSet::new(),
        }
    }

    fn best(
        &self,
        mut found: Vec<(Rank, NodeId)>,
        fits: impl Fn(NodeId) -> bool,
    ) -> Option<NodeId> {
        found.sort_by_key(|(rank, _)| *rank);
        found
            .into_iter()
            .map(|(_, node)| node)
            .find(|node| !self.used.contains(node) && fits(*node))
    }

    fn set(&mut self, bone: HumanBone, node: NodeId) {
        self.humanoid.set_bone(bone, Some(node));
        self.used.insert(node);
    }

    fn limb(&mut self, limb: Limb, side: Side, fits: impl Fn(NodeId) -> bool) -> Option<NodeId> {
        let mut found = self
            .candidates
            .limbs
            .get(&(limb, side))
            .cloned()
            .unwrap_or_default();
        if limb.is_link() {
            for (rank, _) in &mut found {
                *rank = Rank {
                    priority: rank.depth as usize,
                    ..*rank
                };
            }
        }
        let node = self.best(found, fits)?;
        self.set(limb.bone(side), node);
        Some(node)
    }

    fn chain(&mut self, side: Side, limbs: [Limb; 3]) -> Option<[NodeId; 3]> {
        let tree = self.tree;
        let upper = self.limb(limbs[0], side, |_| true)?;
        let lower = self.limb(limbs[1], side, |node| tree.below(upper, node))?;
        let end = self.limb(limbs[2], side, |node| tree.below(lower, node))?;
        Some([upper, lower, end])
    }

    fn place(mut self) -> Option<Humanoid> {
        let tree = self.tree;
        let arms = [Side::Left, Side::Right]
            .map(|side| self.chain(side, [Limb::UpperArm, Limb::LowerArm, Limb::Hand]));
        let legs = [Side::Left, Side::Right]
            .map(|side| self.chain(side, [Limb::UpperLeg, Limb::LowerLeg, Limb::Foot]));
        let [Some(left_arm), Some(right_arm)] = arms else {
            return None;
        };
        let [Some(left_leg), Some(right_leg)] = legs else {
            return None;
        };
        let shoulders = tree.common(left_arm[0], right_arm[0])?;
        let found = std::mem::take(&mut self.candidates.head);
        let head = self.best(found, |node| {
            !tree.contains(left_arm[0], node)
                && !tree.contains(right_arm[0], node)
                && tree.common(shoulders, node).is_some()
        })?;
        self.set(HumanBone::Head, head);
        let top = tree.common(shoulders, head)?;
        let hips = tree.common(tree.common(left_leg[0], right_leg[0])?, top)?;
        let spine = tree.path(hips, top);
        if spine.is_empty()
            || [left_arm, right_arm, left_leg, right_leg]
                .iter()
                .flatten()
                .any(|node| spine.contains(node) || *node == hips)
            || [left_leg[0], right_leg[0]]
                .iter()
                .any(|leg| tree.contains(top, *leg))
            || [left_arm[0], right_arm[0]]
                .iter()
                .any(|arm| !tree.below(top, *arm))
            || tree.contains(left_arm[0], right_arm[0])
            || tree.contains(right_arm[0], left_arm[0])
        {
            return None;
        }
        self.set(HumanBone::Hips, hips);
        self.set(HumanBone::Spine, spine[0]);
        if spine.len() >= 2 {
            self.set(HumanBone::Chest, spine[1]);
        }
        if spine.len() >= 3 {
            self.set(HumanBone::UpperChest, spine[spine.len() - 1]);
        }
        let neck = tree.path(top, head);
        if neck.len() >= 2 {
            self.set(HumanBone::Neck, neck[0]);
        }
        for (side, arm, leg) in [
            (Side::Left, left_arm, left_leg),
            (Side::Right, right_arm, right_leg),
        ] {
            self.limb(Limb::Shoulder, side, |node| {
                tree.below(top, node) && tree.below(node, arm[0])
            });
            self.limb(Limb::Toes, side, |node| tree.below(leg[2], node));
            self.limb(Limb::Eye, side, |node| tree.below(head, node));
            for digit in DIGITS {
                self.digit(digit, side, arm[2]);
            }
        }
        let found = std::mem::take(&mut self.candidates.jaw);
        if let Some(jaw) = self.best(found, |node| tree.below(head, node)) {
            self.set(HumanBone::Jaw, jaw);
        }
        Some(self.humanoid)
    }

    fn digit(&mut self, digit: Digit, side: Side, hand: NodeId) {
        let tree = self.tree;
        let Some(mut phalanges) = self.candidates.digits.remove(&(digit, side)) else {
            return;
        };
        phalanges.retain(|phalanx| {
            tree.below(hand, phalanx.node)
                && !self.used.contains(&phalanx.node)
                && (digit.is_thumb() || !phalanx.metacarpal)
        });
        phalanges.sort_by_key(|phalanx| (phalanx.order, tree.depth(phalanx.node)));
        if phalanges.len() > JOINTS {
            phalanges.retain(|phalanx| !self.leaf[phalanx.node.index()]);
        }
        let excess = phalanges.len().saturating_sub(JOINTS);
        let kept = match digit.is_thumb() {
            true => &phalanges[..phalanges.len() - excess],
            false => &phalanges[excess..],
        };
        let mut above = hand;
        for (bone, phalanx) in digit.bones(side).into_iter().zip(kept) {
            if !tree.below(above, phalanx.node) {
                return;
            }
            self.set(bone, phalanx.node);
            above = phalanx.node;
        }
    }
}
