use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn from_dos(value: zip::DateTime) -> Option<SystemTime> {
    if !value.is_valid() {
        return None;
    }
    let days = days_from_civil(
        i64::from(value.year()),
        u32::from(value.month()),
        u32::from(value.day()),
    );
    let seconds = days * 86_400
        + i64::from(value.hour()) * 3_600
        + i64::from(value.minute()) * 60
        + i64::from(value.second());
    Some(from_unix_seconds(seconds))
}

pub(crate) fn to_dos(time: SystemTime) -> Option<zip::DateTime> {
    let seconds = to_unix_seconds(time);
    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    zip::DateTime::from_date_and_time(
        u16::try_from(year).ok()?,
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
        u8::try_from(time_of_day / 3_600).ok()?,
        u8::try_from((time_of_day % 3_600) / 60).ok()?,
        u8::try_from(time_of_day % 60).ok()?,
    )
    .ok()
}

pub(crate) fn from_unix_seconds(seconds: i64) -> SystemTime {
    let span = Duration::from_secs(seconds.unsigned_abs());
    match seconds >= 0 {
        true => UNIX_EPOCH + span,
        false => UNIX_EPOCH - span,
    }
}

pub(crate) fn to_unix_seconds(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_secs().cast_signed(),
        Err(before) => {
            let before = before.duration();
            -(before.as_secs().cast_signed()) - i64::from(before.subsec_nanos() > 0)
        }
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };

    #[expect(
        clippy::cast_sign_loss,
        reason = "month and day are positive by construction"
    )]
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}
