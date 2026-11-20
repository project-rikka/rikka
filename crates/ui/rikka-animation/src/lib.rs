mod channel;
mod duration;
mod easing;
mod frame_clock;
mod motion;
mod presence;
mod scheme;
mod spring;

pub use channel::Channel;
pub use duration::DurationToken;
pub use easing::Easing;
pub use frame_clock::FrameClock;
pub use motion::{Curve, Motion};
pub use presence::Presence;
pub use scheme::{MotionScheme, Property, Speed};
pub use spring::Spring;
