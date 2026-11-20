use jiff::civil::{Date, DateTime};

pub(crate) const DAY: i64 = 86_400;

pub(crate) fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

pub(crate) fn days_in_year(year: i64) -> u32 {
    if is_leap(year) { 366 } else { 365 }
}

pub(crate) fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

pub(crate) fn weekday_from_days(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32
}

pub(crate) fn datetime_seconds(datetime: DateTime) -> i64 {
    days_from_civil(
        i64::from(datetime.year()),
        datetime.month() as u32,
        datetime.day() as u32,
    ) * DAY
        + i64::from(datetime.hour()) * 3600
        + i64::from(datetime.minute()) * 60
        + i64::from(datetime.second())
}

pub(crate) fn date_seconds(date: Date) -> i64 {
    days_from_civil(
        i64::from(date.year()),
        date.month() as u32,
        date.day() as u32,
    ) * DAY
}

pub(crate) fn datetime_from_seconds(seconds: i64) -> Option<DateTime> {
    let days = seconds.div_euclid(DAY);
    let within = seconds.rem_euclid(DAY);
    let (year, month, day) = civil_from_days(days);
    DateTime::new(
        i16::try_from(year).ok()?,
        month as i8,
        day as i8,
        (within / 3600) as i8,
        (within % 3600 / 60) as i8,
        (within % 60) as i8,
        0,
    )
    .ok()
}

pub(crate) fn date_from_seconds(seconds: i64) -> Option<Date> {
    datetime_from_seconds(seconds).map(DateTime::date)
}
