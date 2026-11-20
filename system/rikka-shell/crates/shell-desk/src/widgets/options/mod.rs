mod clock;
mod folder;
mod monitor;
mod photos;
mod weather;
mod zones;

use std::rc::Rc;

use desk_layout::{Calendar, Content, FirstDay, Media, Note, Power, Widget};
use rikka_ui::{Element, Label, Length, Styled, View, text, tr};
use rikka_widgets::{ControlSize, button, combo, field, switch};

use crate::widgets::live::Live;

pub(crate) type Change = Rc<dyn Fn(Content)>;

const PICKER: f32 = 180.0;

#[must_use]
pub fn options(widget: &Widget, live: &Live, change: Rc<dyn Fn(Content)>) -> Element {
    match &widget.content {
        Content::Clock(clock) => clock::options(clock, &change),
        Content::Calendar(calendar) => calendar_options(calendar, &change),
        Content::Weather(weather) => weather::options(weather, live.sources(), &change),
        Content::Media(media) => media_options(media, &change),
        Content::Monitor(monitor) => monitor::options(monitor, &change),
        Content::Power(power) => power_options(power, &change),
        Content::Note(note) => note_options(note, &change),
        Content::Photos(photos) => photos::options(photos, &change),
    }
}

pub(crate) fn edited<T: Clone + 'static, V>(
    change: &Change,
    current: &T,
    wrap: fn(T) -> Content,
    apply: impl Fn(&mut T, V) + 'static,
) -> impl Fn(V) + 'static {
    let (change, current) = (Rc::clone(change), current.clone());
    move |value| {
        let mut next = current.clone();
        apply(&mut next, value);
        change(wrap(next));
    }
}

#[must_use]
pub(crate) fn toggle(label: Label, on: bool, set: impl Fn(bool) + 'static) -> Element {
    field(label, switch(on).on_toggle(move |on: bool| set(on))).build()
}

#[must_use]
pub(crate) fn chooser(
    label: Label,
    choices: impl IntoIterator<Item = Label>,
    chosen: usize,
    choose: impl Fn(usize) + 'static,
) -> Element {
    field(
        label,
        combo(chosen)
            .options(choices)
            .control_size(ControlSize::Small)
            .on_select(move |index: usize| choose(index))
            .width(Length::px(PICKER)),
    )
    .build()
}

fn calendar_options(calendar: &Calendar, change: &Change) -> Element {
    const DAYS: [FirstDay; 4] = [
        FirstDay::Locale,
        FirstDay::Sunday,
        FirstDay::Monday,
        FirstDay::Saturday,
    ];
    let change = Rc::clone(change);
    chooser(
        tr!("shell.desk.widget.options.first-day"),
        [
            tr!("shell.desk.widget.options.first-day-region"),
            tr!("shell.desk.widget.options.sunday"),
            tr!("shell.desk.widget.options.monday"),
            tr!("shell.desk.widget.options.saturday"),
        ],
        DAYS.iter()
            .position(|day| *day == calendar.first_day)
            .unwrap_or_default(),
        move |index| {
            if let Some(first_day) = DAYS.get(index).copied() {
                change(Content::Calendar(Calendar { first_day }));
            }
        },
    )
}

fn media_options(media: &Media, change: &Change) -> Element {
    let change = Rc::clone(change);
    toggle(
        tr!("shell.desk.widget.options.waveform"),
        media.waveform,
        move |waveform| change(Content::Media(Media { waveform })),
    )
}

fn power_options(power: &Power, change: &Change) -> Element {
    let change = Rc::clone(change);
    toggle(
        tr!("shell.desk.widget.options.devices"),
        power.devices,
        move |devices| change(Content::Power(Power { devices })),
    )
}

fn note_options(note: &Note, change: &Change) -> Element {
    let change = Rc::clone(change);
    field(
        tr!("shell.desk.widget.options.note"),
        button(text(tr!("shell.desk.widget.options.clear")))
            .danger()
            .control_size(ControlSize::Small)
            .enabled(!note.text.is_empty())
            .on_press(move || change(Content::Note(Note::default()))),
    )
    .build()
}
