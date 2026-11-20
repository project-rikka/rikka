use std::time::Duration;

use jiff::Zoned;
use jiff::civil::Date;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::home::HomePage;
use crate::internal::days;

const MINUTE: Duration = Duration::from_secs(60);
const SLACK: Duration = Duration::from_millis(50);
const TIME: f32 = 72.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Minute {
    date: Date,
    hour: i8,
    minute: i8,
}

impl Minute {
    pub(crate) fn now() -> (Minute, Duration) {
        let now = Zoned::now();
        let minute = Minute {
            date: now.date(),
            hour: now.hour(),
            minute: now.minute(),
        };
        let into = Duration::new(
            u64::try_from(now.second()).unwrap_or(0),
            u32::try_from(now.subsec_nanosecond()).unwrap_or(0),
        );
        (minute, MINUTE.saturating_sub(into) + SLACK)
    }

    fn time(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

pub(crate) fn view(page: &HomePage) -> Element {
    let now = page.now();
    column! {
        text(now.time()).display().light().text_size(TIME).no_wrap();
        text(days::weekday_and_date(now.date)).body().muted();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .any()
}
