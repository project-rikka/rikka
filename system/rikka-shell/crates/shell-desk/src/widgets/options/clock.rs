use std::rc::Rc;

use desk_layout::{Clock, Content, Face};
use rikka_ui::{Element, Length, Space, Styled, TextStyled, View, text, tr};
use rikka_widgets::{ControlSize, column, field, segmented};

use crate::widgets::options::zones::zones;
use crate::widgets::options::{Change, edited, toggle};

const FACES: [Face; 2] = [Face::Digital, Face::Analog];
const SEGMENTS: f32 = 180.0;

#[must_use]
pub(crate) fn options(clock: &Clock, change: &Change) -> Element {
    let faced = edited(change, clock, Content::Clock, |clock, index: usize| {
        clock.face = FACES[index.min(FACES.len() - 1)];
    });
    let seconds = edited(change, clock, Content::Clock, |clock, seconds: bool| {
        clock.seconds = seconds;
    });
    let zoned = Rc::new(edited(
        change,
        clock,
        Content::Clock,
        |clock, zone: Option<String>| clock.zone = zone,
    ));
    column! {
        field(
            tr!("shell.desk.widget.options.face"),
            segmented(FACES.iter().position(|face| *face == clock.face).unwrap_or_default())
                .segments([
                    tr!("shell.desk.widget.options.digital"),
                    tr!("shell.desk.widget.options.analog"),
                ])
                .control_size(ControlSize::Small)
                .on_select(move |index: usize| faced(index))
                .width(Length::px(SEGMENTS)),
        );
        toggle(tr!("shell.desk.widget.options.seconds"), clock.seconds, seconds);
        text(tr!("shell.desk.widget.options.zone")).label().muted();
        zones(clock.zone.clone(), zoned);
    }
    .gap(Space::NORMAL)
    .fill_width()
    .build()
}
