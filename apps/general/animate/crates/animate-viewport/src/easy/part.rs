use animate_body::{HumanBone, Side};
use animate_motion::Part;
use animate_solve::{Finger, Handle};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Marker {
    Handle(Handle),
    Finger(Finger),
    Eyes,
    Mouth,
}

impl Marker {
    pub(super) fn handle(self) -> Option<Handle> {
        match self {
            Self::Handle(handle) => Some(handle),
            Self::Finger(_) | Self::Eyes | Self::Mouth => None,
        }
    }

    pub(super) fn side(self) -> Option<Side> {
        match self {
            Self::Handle(handle) => handle.side(),
            Self::Finger(finger) => Some(finger.side()),
            Self::Eyes | Self::Mouth => None,
        }
    }

    pub(super) fn parent(self) -> Option<Self> {
        let handle = match self {
            Self::Eyes | Self::Mouth => return None,
            Self::Finger(finger) => Handle::Hand(finger.side()),
            Self::Handle(handle) => match handle {
                Handle::Centre => return None,
                Handle::LowerBody | Handle::UpperBody => Handle::Centre,
                Handle::Head | Handle::Shoulder(_) => Handle::UpperBody,
                Handle::UpperArm(side) => Handle::Shoulder(side),
                Handle::Forearm(side) => Handle::UpperArm(side),
                Handle::Hand(side) => Handle::Forearm(side),
                Handle::Foot(_) => Handle::LowerBody,
                Handle::Toes(side) => Handle::Foot(side),
            },
        };
        Some(Self::Handle(handle))
    }
}

pub(super) fn end_of(part: Part) -> Option<HumanBone> {
    match part {
        Part::Root => Some(HumanBone::Hips),
        Part::Head => Some(HumanBone::Head),
        _ => part.limb().map(|limb| limb.chain()[2]),
    }
}

pub(super) fn part_of(marker: Marker) -> Option<Part> {
    Some(match marker {
        Marker::Eyes => Part::Gaze,
        Marker::Mouth => return None,
        Marker::Finger(finger) => fingers(finger.side()),
        Marker::Handle(handle) => match handle {
            Handle::Centre | Handle::LowerBody => Part::Root,
            Handle::UpperBody => Part::Torso,
            Handle::Head => Part::Head,
            _ => handle.limb().map_or(Part::Root, Part::from),
        },
    })
}

pub(super) fn fingers(side: Side) -> Part {
    match side {
        Side::Left => Part::LeftFingers,
        Side::Right => Part::RightFingers,
    }
}

pub(super) fn holds_hand(part: Part, side: Side) -> bool {
    part == fingers(side) || part.limb() == Some(side.arm())
}
