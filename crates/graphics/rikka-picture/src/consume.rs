use std::time::Duration;

use rikka_gpu::{ComputedFrame, VideoDeviceHandles};
use rikka_types::Size;

use crate::imported::ImportedFrame;
use crate::video::VideoState;

pub use crate::feed::{Arrival, Content};
pub use crate::handoff::{announce_arrival, on_arrival};
pub use crate::repaint::DamageLog;
pub use crate::video::VideoCommand;

pub trait FeedConsumer {
    fn attach(&self, handles: Option<VideoDeviceHandles>);

    fn is_attached(&self) -> bool;

    fn is_abandoned(&self) -> bool;

    fn take(&self) -> Option<Arrival>;
}

pub trait ImportedConsumer {
    fn take(&self) -> Option<ImportedFrame>;
}

pub trait ComputedConsumer {
    fn is_abandoned(&self) -> bool;

    fn upload_now(&self, upload: impl FnOnce(ComputedFrame));

    fn upload_when_ready(&self, upload: impl FnOnce(ComputedFrame));
}

pub trait VideoConsumer {
    fn take_commands(&self) -> Vec<VideoCommand>;

    fn set_size(&self, size: Size);

    fn set_state(&self, state: VideoState);

    fn set_position(&self, position: Duration);

    fn set_duration(&self, duration: Option<Duration>);

    fn fail(&self, error: String);
}
