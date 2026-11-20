use jiff::civil::Weekday;
use rikka_ui::{Label, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameLength {
    Short,
    Long,
}

#[must_use]
pub fn weekday(weekday: Weekday, spelled: NameLength) -> Label {
    let length = match spelled {
        NameLength::Short => "short",
        NameLength::Long => "long",
    };
    tr!(&format!(
        "shell.island.weekday.{length}.{}",
        day_key(weekday)
    ))
}

#[must_use]
pub(crate) fn day_key(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "mon",
        Weekday::Tuesday => "tue",
        Weekday::Wednesday => "wed",
        Weekday::Thursday => "thu",
        Weekday::Friday => "fri",
        Weekday::Saturday => "sat",
        Weekday::Sunday => "sun",
    }
}
