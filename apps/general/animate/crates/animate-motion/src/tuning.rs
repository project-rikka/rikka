pub(crate) use rikka_mmd::FRAMES_PER_SECOND;

pub(crate) const SETTLE_PERIOD: f32 = 0.4;

pub(crate) const SETTLE_DECAY: f32 = 0.15;

pub(crate) const SETTLE_TAIL: f32 = 0.5;

pub(crate) const HIT_SLOPE: f32 = 1.5;

pub(crate) const FLAT_TURN: f32 = 1e-6;

pub(crate) const FRAME_SECONDS: f32 = 1.0 / FRAMES_PER_SECOND;

pub(crate) const SECONDARY_STIFF_HZ: f32 = 12.0;

pub(crate) const SECONDARY_SOFT_HZ: f32 = 3.0;

pub(crate) const SECONDARY_DAMPING: f32 = 0.5;

pub(crate) const SECONDARY_REACH_DEGREES: f32 = 25.0;

pub(crate) const SECONDARY_TAIL_FRAMES: u32 = 30;

pub(crate) const BLINK_GAP_SECONDS: (f32, f32) = (2.0, 6.0);

pub(crate) const BLINK_CLOSING_FRAMES: f32 = 2.0;

pub(crate) const BLINK_OPENING_FRAMES: f32 = 3.0;

pub(crate) const DOUBLE_BLINK_CHANCE: f32 = 0.1;

pub(crate) const DOUBLE_BLINK_FRAMES: u32 = 7;

pub(crate) const BLINK_HELD: f32 = 0.5;

pub(crate) const CHUNK_FRAMES: u32 = 64;

pub(crate) const BAKE_TAIL_FRAMES: u32 = 30;

pub(crate) const CONTACT_HEIGHT: f32 = 0.02;

pub(crate) const CONTACT_SPEED: f32 = 0.01;

pub(crate) const CONTACT_FRAMES: usize = 3;

pub(crate) const CONTACT_DRIFT: f32 = 0.01;

pub(crate) const REDUCE_DEGREES: f32 = 0.5;

pub(crate) const REDUCE_POSITION: f32 = 0.002;

pub(crate) const FIT_RIDGE: f32 = 1e-3;
