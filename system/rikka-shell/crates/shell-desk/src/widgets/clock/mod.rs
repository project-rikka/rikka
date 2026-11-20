mod analog;
mod dial;
mod digital;
mod zone;

use desk_layout::{Clock, Face, Size};
use jiff::{Timestamp, Zoned};
use rikka_ui::{Align, Element, Inset, Label, Length, Styled, TextStyled, View, palette, text, tr};
use rikka_widgets::row;
use shell_feeds::{NameLength, dates, weekday};
use shell_kit::{Period, WallClock};
use zone::Resolved;
pub(crate) use zone::{Offset, city, offset};

use crate::widgets::kit::{Header, display, header};
use crate::widgets::paint::{Ruler, ruler};
use crate::widgets::sources::Sources;

const DESCENT: f32 = 0.2;
const SECONDS_OF_DAY: f32 = 86_400.0;

pub(crate) struct Reading {
    wall: WallClock,
    resolved: Resolved,
    at: Timestamp,
    zoned: Zoned,
    seconds: bool,
}

#[must_use]
pub(crate) fn view(clock: &Clock, size: Size, sources: &Sources) -> Element {
    let resolved = Resolved::resolve(clock.zone.as_deref());
    if matches!(resolved, Resolved::Pending) {
        return Element::empty();
    }
    let ticking = match clock.seconds && clock.face == Face::Digital {
        true => Period::Second,
        false => Period::Minute,
    };
    let wall = sources.feeds.wall.clone();
    let at = wall.now(ticking);
    let reading = Reading {
        wall,
        zoned: at.to_zoned(resolved.time_zone()),
        resolved,
        at,
        seconds: clock.seconds,
    };
    match clock.face {
        Face::Digital => digital::view(&reading, size),
        Face::Analog => analog::view(&reading, size),
    }
}

impl Reading {
    fn headline(&self) -> Label {
        match self.resolved.city() {
            Some(city) => Label::from(city),
            None => weekday(self.zoned.weekday(), NameLength::Long),
        }
    }

    fn media_header(&self) -> Header {
        header(None, self.headline())
    }

    fn kicker_or(&self, at_home: Label) -> Header {
        self.media_header()
            .trailing(self.resolved.relative(self.at).unwrap_or(at_home))
    }

    fn week(&self) -> Label {
        dates::week(self.zoned.date())
    }

    fn week_and_day(&self) -> Label {
        tr!(
            "shell.desk.widget.clock.week-day",
            week = self.zoned.date().iso_week_date().week(),
            day = self.zoned.date().day_of_year(),
        )
    }

    fn footnote(&self) -> Label {
        match self.resolved.relative(self.at) {
            Some(relative) => relative,
            None => dates::long_date(self.zoned.date()),
        }
    }

    fn dateline(&self) -> Label {
        match self.resolved.city() {
            Some(_) => self.footnote(),
            None => dates::day_month(self.zoned.date()),
        }
    }

    fn long_date(&self) -> Label {
        dates::long_date(self.zoned.date())
    }

    fn through_day(&self) -> f32 {
        let seconds = i32::from(self.zoned.hour()) * 3600
            + i32::from(self.zoned.minute()) * 60
            + i32::from(self.zoned.second());
        seconds as f32 / SECONDS_OF_DAY
    }

    fn day_ruler(&self, labelled: bool) -> Element {
        ruler(self.through_day())
            .labelled(labelled)
            .height(Length::px(Ruler::height(labelled)))
            .fill_width()
            .build()
    }

    fn time(&self, size: f32, seconds: f32) -> Element {
        let shown = (self.seconds && seconds > 0.0).then(|| format!(":{:02}", self.zoned.second()));
        row! {
            display(dates::time(self.zoned.hour(), self.zoned.minute()), size);
            if let Some(shown) = shown {
                display(shown, seconds)
                    .color(palette().text_muted)
                    .margin(Inset::exact(0.0, 0.0, (size - seconds) * DESCENT, 0.0));
            }
        }
        .align_items(Align::End)
        .build()
    }

    fn date_line(&self) -> Element {
        text(self.dateline()).label().muted().no_wrap().build()
    }
}
