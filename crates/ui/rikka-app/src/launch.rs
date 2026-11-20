use rikka_appearance::Appearance;
use rikka_theme::Theme;
use rikka_ui_host::VideoPlayback;
use rikka_worker::EventFd;

use crate::device::Device;
use crate::instance::Policy;
use crate::tasks::{LoopProxy, LoopReceiver};
use crate::window::Opening;

pub(crate) struct Launch {
    pub(crate) app_name: String,
    pub(crate) device: Device,
    pub(crate) videos: Option<Box<dyn VideoPlayback>>,
    pub(crate) theme: Theme,
    pub(crate) appearance: Option<Appearance>,
    #[cfg(unix)]
    pub(crate) language: bool,
    pub(crate) windows: Vec<Opening>,
    pub(crate) policy: Option<Policy>,
    #[cfg(unix)]
    pub(crate) arriving: Option<String>,
    pub(crate) proxy: LoopProxy,
    pub(crate) mailbox: LoopReceiver,
    pub(crate) wake: EventFd,
}
