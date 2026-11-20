mod hands;
mod legs;
mod torso;

use rikka_model_core::glam::Vec3;
use rikka_model_core::{HumanBone, Humanoid, NodeId};

use crate::facing::Facing;
use crate::frame::Frame;
use legs::Leg;
use rikka_model_core::RigFacts;

#[derive(Clone, Debug)]
pub(crate) struct Recognized {
    pub(crate) humanoid: Humanoid,
    pub(crate) facing: Facing,
}

pub(crate) fn recognize(frame: &Frame, evidence: &RigFacts, named: Option<Humanoid>) -> Recognized {
    if let Some(declared) = &evidence.humanoid {
        return declared_as_is(frame, declared.clone());
    }
    let structural = structure(frame, evidence);
    match named {
        Some(named) => declared_as_is(frame, completed(named, &structural.humanoid)),
        None => structural,
    }
}

fn declared_as_is(frame: &Frame, humanoid: Humanoid) -> Recognized {
    let midline = humanoid
        .bone(HumanBone::Hips)
        .map_or(frame.centre, |hips| frame.origin(hips));
    Recognized {
        facing: Facing::new(declared_forward(frame, &humanoid), midline),
        humanoid,
    }
}

fn completed(mut named: Humanoid, structural: &Humanoid) -> Humanoid {
    let agrees = HumanBone::ALL
        .iter()
        .filter(|bone| bone.is_required() && !matches!(bone, HumanBone::Hips | HumanBone::Spine))
        .all(|bone| named.bone(*bone) == structural.bone(*bone));
    if !agrees {
        return named;
    }
    let trunk = [
        HumanBone::Hips,
        HumanBone::Spine,
        HumanBone::Chest,
        HumanBone::UpperChest,
        HumanBone::Neck,
        HumanBone::Head,
    ];
    for (bone, node) in structural.bones().filter(|(bone, _)| !trunk.contains(bone)) {
        let taken = named.bones().any(|(_, named_node)| named_node == node);
        if named.bone(bone).is_none() && !taken {
            named.set_bone(bone, Some(node));
        }
    }
    named
}

fn structure(frame: &Frame, evidence: &RigFacts) -> Recognized {
    let legs = legs::recognize(frame, evidence);
    let uppers: Vec<NodeId> = legs.iter().flatten().map(|leg| leg.upper).collect();
    let hands = hands::recognize(frame, &uppers);
    let torso = torso::recognize(frame, &uppers, hands.as_ref());
    let forward = legs
        .as_ref()
        .and_then(|legs| toward_toes(frame, legs.iter().map(|leg| (leg.foot, leg.toes))))
        .or_else(|| {
            let torso = torso.as_ref()?;
            toward_eyes(frame, torso.head, torso.eyes?)
        })
        .unwrap_or(Vec3::Z);
    let midline = torso
        .as_ref()
        .map_or(frame.centre, |torso| frame.origin(torso.hips));
    let mut placing = Recognition {
        frame,
        humanoid: Humanoid::default(),
        facing: Facing::new(forward, midline),
    };
    if let Some(legs) = legs {
        for leg in legs {
            placing.leg(&leg);
        }
    }
    if let Some(torso) = &torso {
        placing.torso(torso);
    }
    if let (Some(hands), Some(torso)) = (hands, &torso)
        && let Some(chest) = torso.chest
    {
        for hand in hands {
            placing.arm(chest, hand);
        }
    }
    Recognized {
        humanoid: placing.humanoid,
        facing: placing.facing,
    }
}

pub(crate) struct Recognition<'f> {
    frame: &'f Frame,
    humanoid: Humanoid,
    facing: Facing,
}

impl Recognition<'_> {
    pub(crate) fn lateral(&self, node: NodeId) -> f32 {
        self.facing.lateral(self.frame.origin(node))
    }

    fn sided(&self, node: NodeId, left: HumanBone, right: HumanBone) -> HumanBone {
        match self.lateral(node) >= 0.0 {
            true => left,
            false => right,
        }
    }

    fn set(&mut self, bone: HumanBone, node: Option<NodeId>) {
        let Some(node) = node else {
            return;
        };
        let taken = self
            .humanoid
            .bones()
            .any(|(_, bone_node)| bone_node == node);
        if !taken && self.humanoid.bone(bone).is_none() {
            self.humanoid.set_bone(bone, Some(node));
        }
    }

    fn leg(&mut self, leg: &Leg) {
        use HumanBone as B;
        let side = |placing: &Self, left, right| placing.sided(leg.upper, left, right);
        let bones = [
            (
                side(self, B::LeftUpperLeg, B::RightUpperLeg),
                Some(leg.upper),
            ),
            (side(self, B::LeftLowerLeg, B::RightLowerLeg), leg.lower),
            (side(self, B::LeftFoot, B::RightFoot), Some(leg.foot)),
            (side(self, B::LeftToes, B::RightToes), leg.toes),
        ];
        for (bone, node) in bones {
            self.set(bone, node);
        }
    }

    fn torso(&mut self, torso: &torso::Torso) {
        use HumanBone as B;
        self.set(B::Hips, Some(torso.hips));
        let spine = &torso.spine;
        self.set(B::Spine, spine.first().copied());
        if spine.len() >= 2 {
            self.set(B::Chest, Some(spine[1]));
        }
        if spine.len() >= 3 {
            self.set(B::UpperChest, spine.last().copied());
        }
        self.set(B::Neck, torso.neck);
        self.set(B::Head, Some(torso.head));
        if let Some([a, b]) = torso.eyes {
            let (left, right) = match self.lateral(a) >= self.lateral(b) {
                true => (a, b),
                false => (b, a),
            };
            self.set(B::LeftEye, Some(left));
            self.set(B::RightEye, Some(right));
        }
    }

    fn arm(&mut self, chest: NodeId, hand: NodeId) {
        use HumanBone as B;
        let frame = self.frame;
        let side = |left, right| self.sided(hand, left, right);
        let (shoulder, upper, lower) = (
            side(B::LeftShoulder, B::RightShoulder),
            side(B::LeftUpperArm, B::RightUpperArm),
            side(B::LeftLowerArm, B::RightLowerArm),
        );
        let hand_bone = side(B::LeftHand, B::RightHand);
        let between = frame.joints(&frame.path(chest, hand));
        let reach = &between[..between.len().saturating_sub(1)];
        if let Some((at, upper_node, lower_node)) = bend(frame, reach, hand) {
            if at > 0 {
                self.set(shoulder, Some(reach[at - 1]));
            }
            self.set(upper, Some(upper_node));
            self.set(lower, lower_node);
        }
        self.set(hand_bone, Some(hand));
        hands::fingers(self, hand);
    }
}

pub(crate) fn bend(
    frame: &Frame,
    reach: &[NodeId],
    end: NodeId,
) -> Option<(usize, NodeId, Option<NodeId>)> {
    let tip = frame.origin(end);
    let mut best: Option<(f32, usize, usize)> = None;
    for upper in 0..reach.len() {
        for lower in upper + 1..reach.len() {
            let near = frame
                .origin(reach[upper])
                .distance(frame.origin(reach[lower]));
            let far = frame.origin(reach[lower]).distance(tip);
            let balance = near.min(far) / near.max(far).max(1e-6);
            if best.is_none_or(|(best_balance, _, _)| balance > best_balance) {
                best = Some((balance, upper, lower));
            }
        }
    }
    match best {
        Some((_, upper, lower)) => Some((upper, reach[upper], Some(reach[lower]))),
        None => reach.first().map(|upper| (0, *upper, None)),
    }
}

fn toward_toes(
    frame: &Frame,
    feet: impl Iterator<Item = (NodeId, Option<NodeId>)>,
) -> Option<Vec3> {
    let along: Vec3 = feet
        .filter_map(|(foot, toes)| Some(frame.origin(toes?) - frame.origin(foot)))
        .sum();
    flat(along)
}

fn toward_eyes(frame: &Frame, head: NodeId, eyes: [NodeId; 2]) -> Option<Vec3> {
    let middle = (frame.origin(eyes[0]) + frame.origin(eyes[1])) * 0.5;
    flat(middle - frame.origin(head))
}

fn flat(along: Vec3) -> Option<Vec3> {
    let flat = Vec3::new(along.x, 0.0, along.z);
    (flat.length() > 1e-4).then(|| flat.normalize())
}

fn declared_forward(frame: &Frame, humanoid: &Humanoid) -> Vec3 {
    use HumanBone as B;
    let feet = [(B::LeftFoot, B::LeftToes), (B::RightFoot, B::RightToes)]
        .into_iter()
        .filter_map(|(foot, toes)| Some((humanoid.bone(foot)?, humanoid.bone(toes))));
    let eyes = || {
        let head = humanoid.bone(B::Head)?;
        toward_eyes(
            frame,
            head,
            [humanoid.bone(B::LeftEye)?, humanoid.bone(B::RightEye)?],
        )
    };
    let sides = || {
        let left = frame.origin(humanoid.bone(B::LeftUpperArm)?);
        let right = frame.origin(humanoid.bone(B::RightUpperArm)?);
        flat((left - right).cross(Vec3::Y))
    };
    toward_toes(frame, feet)
        .or_else(eyes)
        .or_else(sides)
        .unwrap_or(Vec3::Z)
}
