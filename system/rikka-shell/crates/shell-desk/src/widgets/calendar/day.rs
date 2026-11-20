use jiff::civil::Date;
use rikka_ui::{Element, Length, Styled, TextAlign, TextRole, TextStyled, View, palette, text};
use rikka_widgets::{Container, Flex, container, row_of};
use shell_feeds::dates;

use crate::widgets::calendar::region::Week;

#[derive(Clone, Copy)]
pub(crate) struct Sheet {
    pub(crate) today: Date,
    pub(crate) shown: Date,
    pub(crate) week: Week,
}

impl Sheet {
    pub(crate) fn mark(self, date: Date, diameter: f32, role: TextRole) -> Container {
        let label = text(date.day().to_string())
            .role(role)
            .align(TextAlign::Center)
            .no_wrap();
        let spot = Length::px(diameter);
        let marked = match date == self.today {
            true => container(label.semibold().color(palette().on_accent))
                .background(palette().accent)
                .radius(diameter / 2.0),
            false => {
                let elsewhere =
                    (date.year(), date.month()) != (self.shown.year(), self.shown.month());
                container(match (elsewhere, self.week.rests(date.weekday())) {
                    (true, _) => label.faint(),
                    (false, true) => label.muted(),
                    (false, false) => label,
                })
            }
        };
        marked.center().width(spot).height(spot)
    }

    #[must_use]
    pub(crate) fn cell(self, date: Date, diameter: f32, role: TextRole) -> Element {
        container(self.mark(date, diameter, role))
            .center()
            .grow(1.0)
            .build()
    }
}

pub(super) fn initials(week: Week) -> Flex {
    row_of(week.days().into_iter().map(move |day| {
        let initial = text(dates::initial(day))
            .caption()
            .semibold()
            .align(TextAlign::Center)
            .no_wrap();
        let initial = match week.rests(day) {
            true => initial.faint(),
            false => initial.muted(),
        };
        container(initial).center().grow(1.0).build()
    }))
}
