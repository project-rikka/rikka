mod atomic;
mod buffered;
mod capture;
mod config;
mod device_watch;
mod engine;
mod error;
mod feed;
mod frame;
mod gains;
mod handle;
mod host;
mod lost;
mod mixer;
mod output;
mod pan;
mod param;
mod play_params;
mod ramp;
mod sender;
mod spawn;
mod stream;
mod suspend;
mod time;
mod track;
mod voice;
mod watch;

#[cfg(unix)]
mod pipewire;
#[cfg(unix)]
use pipewire as backend;
#[cfg(windows)]
mod wasapi;
#[cfg(windows)]
use wasapi as backend;

pub use capture::{
    AudioApp, AudioCapture, CaptureConfig, CaptureDevice, CaptureKind, CaptureSource, Fill,
};
pub use config::EngineConfig;
pub use device_watch::{DeviceDirection, DeviceEvent, DeviceWatch};
pub use engine::AudioEngine;
pub use error::{Error, Result, Role};
pub use feed::AudioFeed;
pub use handle::PlaybackHandle;
pub use output::OutputDevice;
pub use pan::Pan;
pub use play_params::PlayParams;
pub use sender::{Sent, SoundEncoding, SoundSender};
pub use track::Track;
pub use watch::{PlaybackEvent, PlaybackWatch};
