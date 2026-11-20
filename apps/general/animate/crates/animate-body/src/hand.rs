use crate::limb::Side;
use crate::pose::HumanPose;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HandPreset {
    Natural,
    Open,
    Fist,
    Peace,
    Point,
    ThumbsUp,
    Ok,
    Rock,
    Gun,
    Relaxed,
    Pinch,
}

const FIST: [f32; 4] = [0.9, 0.0, 1.0, 0.8];
const TUCKED: [f32; 4] = [0.5, -0.4, 0.5, 0.5];
const STRAIGHT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const SHOWN: f32 = 0.02;

impl HandPreset {
    pub const ALL: [Self; 11] = [
        Self::Natural,
        Self::Open,
        Self::Fist,
        Self::Peace,
        Self::Point,
        Self::ThumbsUp,
        Self::Ok,
        Self::Rock,
        Self::Gun,
        Self::Relaxed,
        Self::Pinch,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Open => "open",
            Self::Fist => "fist",
            Self::Peace => "peace",
            Self::Point => "point",
            Self::ThumbsUp => "thumbsUp",
            Self::Ok => "ok",
            Self::Rock => "rock",
            Self::Gun => "gun",
            Self::Relaxed => "relaxed",
            Self::Pinch => "pinch",
        }
    }

    #[must_use]
    pub fn fingers(self) -> [[f32; 4]; 5] {
        match self {
            Self::Natural => [
                [0.2, 0.0, 0.2, 0.2],
                [0.15, 0.1, 0.2, 0.15],
                [0.2, 0.0, 0.25, 0.2],
                [0.25, 0.05, 0.3, 0.25],
                [0.3, 0.15, 0.35, 0.3],
            ],
            Self::Open => [
                [-0.1, 0.6, 0.0, 0.0],
                [0.0, 0.6, 0.0, 0.0],
                [0.0, 0.1, 0.0, 0.0],
                [0.0, 0.5, 0.0, 0.0],
                [0.0, 0.9, 0.0, 0.0],
            ],
            Self::Fist => [TUCKED, FIST, FIST, FIST, FIST],
            Self::Peace => [
                TUCKED,
                [0.0, 0.4, 0.0, 0.0],
                [0.0, 0.4, 0.0, 0.0],
                FIST,
                FIST,
            ],
            Self::Point => [[0.4, -0.4, 0.4, 0.4], STRAIGHT, FIST, FIST, FIST],
            Self::ThumbsUp => [[-0.1, 0.2, 0.0, 0.0], FIST, FIST, FIST, FIST],
            Self::Ok => [
                [0.4, 0.1, 0.4, 0.3],
                [0.6, 0.0, 0.6, 0.5],
                [0.1, 0.1, 0.1, 0.1],
                [0.1, 0.2, 0.1, 0.1],
                [0.1, 0.4, 0.1, 0.1],
            ],
            Self::Rock => [TUCKED, STRAIGHT, FIST, FIST, [0.0, 0.3, 0.0, 0.0]],
            Self::Gun => [[-0.1, 0.5, 0.0, 0.0], STRAIGHT, FIST, FIST, FIST],
            Self::Relaxed => [
                [0.3, -0.1, 0.3, 0.3],
                [0.4, 0.0, 0.5, 0.4],
                [0.45, 0.0, 0.5, 0.4],
                [0.5, 0.0, 0.55, 0.45],
                [0.55, 0.1, 0.6, 0.5],
            ],
            Self::Pinch => [
                [0.4, 0.1, 0.4, 0.3],
                [0.6, 0.0, 0.6, 0.5],
                [0.6, 0.0, 0.65, 0.5],
                [0.65, 0.0, 0.7, 0.55],
                [0.7, 0.1, 0.75, 0.55],
            ],
        }
    }

    pub fn apply(self, side: Side, pose: &mut HumanPose) {
        self.blend(side, pose, 1.0);
    }

    pub fn blend(self, side: Side, pose: &mut HumanPose, share: f32) {
        let targets = self.fingers().into_iter().flatten();
        for (slot, value) in pose.muscles[side.fingers()].iter_mut().zip(targets) {
            *slot += (value - *slot) * share;
        }
    }

    #[must_use]
    pub fn matching(side: Side, pose: &HumanPose) -> Option<Self> {
        let fingers = &pose.muscles[side.fingers()];
        Self::ALL.into_iter().find(|preset| {
            preset
                .fingers()
                .into_iter()
                .flatten()
                .zip(fingers)
                .all(|(desired, value)| (desired - value).abs() <= SHOWN)
        })
    }
}
