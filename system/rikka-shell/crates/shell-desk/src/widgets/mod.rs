mod about;
mod audio_capture;
mod body;
mod calendar;
mod card;
mod clock;
mod kit;
mod live;
mod media;
mod monitor;
mod needs;
mod note;
mod options;
mod paint;
mod photos;
mod power;
mod quiet;
mod sources;
mod weather;

use std::rc::Rc;

pub use about::about;
use desk_layout::{Content, Widget};
pub use live::Live;
pub use options::options;
use rikka_ui::Element;

use crate::widgets::sources::Sources;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Live,
    Shown,
}

#[must_use]
pub fn widget(widget: &Widget, live: &Live, mode: Mode) -> Element {
    let sources = Rc::clone(live.sources());
    let widget = widget.clone();
    card::card(widget.extent(), mode, move || {
        drawn(&widget, &sources, mode)
    })
}

fn drawn(widget: &Widget, sources: &Sources, mode: Mode) -> Element {
    let (id, size) = (widget.id, widget.size);
    match &widget.content {
        Content::Clock(clock) => clock::view(clock, size, sources),
        Content::Calendar(calendar) => calendar::view(id, calendar, size, sources),
        Content::Weather(weather) => weather::view(weather, size, sources),
        Content::Media(media) => media::view(media, size, sources),
        Content::Monitor(monitor) => monitor::view(monitor, size, sources),
        Content::Power(power) => power::view(power, size, sources),
        Content::Note(note) => note::view(id, note, size, sources, mode),
        Content::Photos(photos) => photos::view(id, photos, size, sources),
    }
}
