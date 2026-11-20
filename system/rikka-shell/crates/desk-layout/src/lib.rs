mod area;
mod content;
mod desks;
mod layout;
mod morph;
mod projection;
mod snap;
mod undo;
mod vacancy;
mod widget;

pub use area::{Area, GAP, MARGIN, Spot};
pub use content::{
    Calendar, Clock, Content, Face, FirstDay, Media, Meter, Monitor, Note, PhotoInterval, Photos,
    Power, Weather,
};
pub use desks::{Desks, ParseError, ParseReport, Skipped};
pub use layout::Layout;
pub use morph::{WidgetMatch, pair};
pub use projection::Projection;
pub use snap::{Guide, REACH, Snapped, Snapper};
pub use undo::Undo;
pub use vacancy::{first_vacant, overlaps, vacant};
pub use widget::{Kind, Size, Widget, WidgetId};
