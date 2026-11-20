use jiff::ToSpan;
use jiff::civil::{Date, Weekday};

#[must_use]
pub(super) fn weeks(month: Date, first: Weekday) -> Vec<[Date; 7]> {
    let opening = month.first_of_month();
    let lead = i64::from(first.until(opening.weekday()));
    let begin = opening.saturating_sub(lead.days());
    let rows = (lead + i64::from(month.days_in_month()) + 6) / 7;
    (0..rows)
        .map(|row| {
            std::array::from_fn(|column| begin.saturating_add((row * 7 + column as i64).days()))
        })
        .collect()
}

#[must_use]
pub(crate) fn week(day: Date, first: Weekday) -> [Date; 7] {
    let begin = day.saturating_sub(i64::from(first.until(day.weekday())).days());
    std::array::from_fn(|column| begin.saturating_add((column as i64).days()))
}

#[must_use]
pub(super) fn paged(today: Date, page: i32) -> Date {
    today.first_of_month().saturating_add(page.months())
}
