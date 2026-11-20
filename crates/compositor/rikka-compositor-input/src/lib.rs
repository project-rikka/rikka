mod access;
mod apply;
mod cursor;
mod device;
mod devices;
mod error;
mod event;
mod leds;
mod notice;
mod plan;
mod queue;
mod report;
mod route;
mod scroll;
mod settings;
mod shared;
mod space;
mod tablet;
mod thread;
mod translate;
mod worker;

pub use access::DeviceAccess;
pub use cursor::{CursorPlacement, CursorSink};
pub use device::{Capabilities, DeviceId, DeviceInfo, PadGroup, PadLayout, PointerKind};
pub use error::{Error, Result};
pub use event::{InputEvent, Switch};
pub use leds::Leds;
pub use notice::InputNotice;
pub use queue::InputQueue;
pub use report::{ConfigReport, Setting, SettingError};
pub use route::{PointerRestrictions, PointerRoute, RouteOutput};
pub use rustix::fs::OFlags;
pub use scroll::ScrollAxis;
pub use settings::{
    Acceleration, ClickMethod, DeviceSettings, MouseScroll, MouseSettings, TapButtons,
    TouchpadScroll, TouchpadSettings, TouchpadUse,
};
pub use shared::PointerShared;
pub use space::{Millimeters, Normalized};
pub use tablet::{ToolEvent, ToolState};
pub use thread::{InputThread, InputThreadBuilder};
