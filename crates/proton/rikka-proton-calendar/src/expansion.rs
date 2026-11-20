use std::collections::VecDeque;

use crate::civil::{
    DAY, civil_from_days, days_from_civil, days_in_month, days_in_year, weekday_from_days,
};
use crate::rule::{Frequency, Rule, RuleEnd, WeekdayNum};

const HORIZON_YEARS: i64 = 400;
const LAST_YEAR: i64 = 9999;

pub(crate) struct Expansion {
    frequency: Frequency,
    interval: i64,
    count: Option<u32>,
    until: Option<i64>,
    start: i64,
    by_second: Vec<u32>,
    by_minute: Vec<u32>,
    by_hour: Vec<u32>,
    rule_seconds: Vec<u32>,
    rule_minutes: Vec<u32>,
    rule_hours: Vec<u32>,
    by_day: Vec<WeekdayNum>,
    by_month_day: Vec<i32>,
    by_year_day: Vec<i32>,
    by_week_no: Vec<i32>,
    by_month: Vec<u32>,
    by_set_pos: Vec<i32>,
    week_start: u32,
    period: i64,
    emitted: u32,
    last: i64,
    started: bool,
    finished: bool,
    pending: VecDeque<i64>,
}

impl Expansion {
    pub(crate) fn new(rule: &Rule, start: i64, all_day: bool, until: Option<i64>) -> Expansion {
        let start_day = start.div_euclid(DAY);
        let (_, start_month, start_dom) = civil_from_days(start_day);
        let within = start.rem_euclid(DAY);
        let frequency = if all_day && rule.frequency < Frequency::Daily {
            Frequency::Daily
        } else {
            rule.frequency
        };

        let mut by_month = rule.by_month.clone();
        let mut by_month_day = rule.by_month_day.clone();
        let mut by_day = rule.by_day.clone();
        let by_week_no = match frequency {
            Frequency::Yearly => rule.by_week_no.clone(),
            _ => Vec::new(),
        };
        if by_week_no.is_empty()
            && rule.by_year_day.is_empty()
            && by_month_day.is_empty()
            && by_day.is_empty()
        {
            match frequency {
                Frequency::Yearly => {
                    if by_month.is_empty() {
                        by_month.push(start_month);
                    }
                    by_month_day.push(start_dom as i32);
                }
                Frequency::Monthly => by_month_day.push(start_dom as i32),
                Frequency::Weekly => by_day.push(WeekdayNum::every(weekday_from_offset(
                    weekday_from_days(start_day),
                ))),
                _ => {}
            }
        }

        let week_start = rule.week_start.to_monday_zero_offset() as u32;
        let period = match frequency {
            Frequency::Yearly => civil_from_days(start_day).0,
            Frequency::Monthly => {
                let (year, month, _) = civil_from_days(start_day);
                year * 12 + i64::from(month) - 1
            }
            Frequency::Weekly => {
                start_day - i64::from((weekday_from_days(start_day) + 7 - week_start) % 7)
            }
            Frequency::Daily => start_day,
            Frequency::Hourly => start.div_euclid(3600),
            Frequency::Minutely => start.div_euclid(60),
            Frequency::Secondly => start,
        };

        let values_or_start = |values: &[u32], fallback: i64| -> Vec<u32> {
            if all_day {
                return vec![0];
            }
            match values.is_empty() {
                true => vec![fallback as u32],
                false => values.to_vec(),
            }
        };

        Expansion {
            frequency,
            interval: i64::from(rule.interval.max(1)),
            count: match rule.end {
                RuleEnd::Count(count) => Some(count),
                _ => None,
            },
            until,
            start,
            by_second: values_or_start(&rule.by_second, within % 60),
            by_minute: values_or_start(&rule.by_minute, within % 3600 / 60),
            by_hour: values_or_start(&rule.by_hour, within / 3600),
            rule_seconds: rule.by_second.clone(),
            rule_minutes: rule.by_minute.clone(),
            rule_hours: rule.by_hour.clone(),
            by_day,
            by_month_day,
            by_year_day: rule.by_year_day.clone(),
            by_week_no,
            by_month,
            by_set_pos: rule.by_set_pos.clone(),
            week_start,
            period,
            emitted: 0,
            last: start,
            started: false,
            finished: false,
            pending: VecDeque::new(),
        }
    }

    pub(crate) fn seek(&mut self, target: i64) {
        if self.count.is_some() || target <= self.start {
            return;
        }
        let day = target.div_euclid(DAY);
        let (distance, unit) = match self.frequency {
            Frequency::Yearly => (civil_from_days(day).0 - self.period, 1),
            Frequency::Monthly => {
                let (year, month, _) = civil_from_days(day);
                (year * 12 + i64::from(month) - 1 - self.period, 1)
            }
            Frequency::Weekly => (day - self.period, 7),
            Frequency::Daily => (day - self.period, 1),
            Frequency::Hourly => (target.div_euclid(3600) - self.period, 1),
            Frequency::Minutely => (target.div_euclid(60) - self.period, 1),
            Frequency::Secondly => (target - self.period, 1),
        };
        let steps = (distance / unit).div_euclid(self.interval) - 1;
        if steps <= 0 {
            return;
        }
        self.period += steps * self.interval * unit;
        self.started = true;
        self.pending.clear();
        let period_start = match self.frequency {
            Frequency::Hourly => self.period * 3600,
            Frequency::Minutely => self.period * 60,
            Frequency::Secondly => self.period,
            _ => self.period_days().0 * DAY,
        };
        self.last = self.last.max(period_start - 1);
    }

    fn period_days(&self) -> (i64, i64, i64) {
        match self.frequency {
            Frequency::Yearly => {
                let year = self.period;
                if self.by_week_no.is_empty() {
                    let first = days_from_civil(year, 1, 1);
                    (first, first + i64::from(days_in_year(year)), year)
                } else {
                    let first = week_one(year, self.week_start);
                    (first, week_one(year + 1, self.week_start), year)
                }
            }
            Frequency::Monthly => {
                let year = self.period.div_euclid(12);
                let month = (self.period.rem_euclid(12) + 1) as u32;
                let first = days_from_civil(year, month, 1);
                (first, first + i64::from(days_in_month(year, month)), year)
            }
            Frequency::Weekly => (self.period, self.period + 7, 0),
            Frequency::Daily => (self.period, self.period + 1, 0),
            Frequency::Hourly => {
                let day = (self.period * 3600).div_euclid(DAY);
                (day, day + 1, 0)
            }
            Frequency::Minutely => {
                let day = (self.period * 60).div_euclid(DAY);
                (day, day + 1, 0)
            }
            Frequency::Secondly => {
                let day = self.period.div_euclid(DAY);
                (day, day + 1, 0)
            }
        }
    }

    fn day_matches(&self, day: i64, period_year: i64) -> bool {
        let (year, month, dom) = civil_from_days(day);
        if !self.by_month.is_empty() && !self.by_month.contains(&month) {
            return false;
        }
        if !self.by_week_no.is_empty() {
            let first = week_one(period_year, self.week_start);
            let weeks = (week_one(period_year + 1, self.week_start) - first) / 7;
            let number = (day - first).div_euclid(7) + 1;
            if !matches_any_index(&self.by_week_no, number, weeks) {
                return false;
            }
        }
        if !self.by_year_day.is_empty() {
            let length = i64::from(days_in_year(year));
            let ordinal = day - days_from_civil(year, 1, 1) + 1;
            if !matches_any_index(&self.by_year_day, ordinal, length) {
                return false;
            }
        }
        if !self.by_month_day.is_empty() {
            let length = i64::from(days_in_month(year, month));
            if !matches_any_index(&self.by_month_day, i64::from(dom), length) {
                return false;
            }
        }
        if !self.by_day.is_empty() {
            let weekday = weekday_from_days(day);
            let matched = self.by_day.iter().any(|rule_day| {
                if rule_day.weekday.to_monday_zero_offset() as u32 != weekday {
                    return false;
                }
                let Some(ordinal) = rule_day.ordinal else {
                    return true;
                };
                let ordinal = i64::from(ordinal);
                let monthly = self.frequency == Frequency::Monthly
                    || (self.frequency == Frequency::Yearly && !self.by_month.is_empty());
                let yearly = self.frequency == Frequency::Yearly && self.by_week_no.is_empty();
                if monthly {
                    let length = i64::from(days_in_month(year, month));
                    let dom = i64::from(dom);
                    if ordinal > 0 {
                        (dom - 1) / 7 + 1 == ordinal
                    } else {
                        -((length - dom) / 7 + 1) == ordinal
                    }
                } else if yearly {
                    let length = i64::from(days_in_year(year));
                    let ordinal_day = day - days_from_civil(year, 1, 1) + 1;
                    if ordinal > 0 {
                        (ordinal_day - 1) / 7 + 1 == ordinal
                    } else {
                        -((length - ordinal_day) / 7 + 1) == ordinal
                    }
                } else {
                    true
                }
            });
            if !matched {
                return false;
            }
        }
        true
    }

    fn period_set(&self) -> Vec<i64> {
        let (first, end, period_year) = self.period_days();
        let mut set = Vec::new();

        for day in first..end {
            if !self.day_matches(day, period_year) {
                continue;
            }
            let base = day * DAY;
            match self.frequency {
                Frequency::Hourly | Frequency::Minutely | Frequency::Secondly => {
                    let period_start = match self.frequency {
                        Frequency::Hourly => self.period * 3600,
                        Frequency::Minutely => self.period * 60,
                        _ => self.period,
                    };
                    let within = period_start - base;
                    let hour = (within / 3600) as u32;
                    let minute = (within % 3600 / 60) as u32;
                    let second = (within % 60) as u32;
                    let passes = |raw: &[u32], value: u32| raw.is_empty() || raw.contains(&value);
                    if !passes(&self.rule_hours, hour) {
                        continue;
                    }
                    let minutes = match self.frequency <= Frequency::Minutely {
                        true if !passes(&self.rule_minutes, minute) => continue,
                        true => vec![minute],
                        false => self.by_minute.clone(),
                    };
                    let seconds = match self.frequency == Frequency::Secondly {
                        true if !passes(&self.rule_seconds, second) => continue,
                        true => vec![second],
                        false => self.by_second.clone(),
                    };
                    for minute in &minutes {
                        for second in &seconds {
                            set.push(
                                base + i64::from(hour) * 3600
                                    + i64::from(*minute) * 60
                                    + i64::from(*second),
                            );
                        }
                    }
                }
                _ => {
                    for hour in &self.by_hour {
                        for minute in &self.by_minute {
                            for second in &self.by_second {
                                set.push(
                                    base + i64::from(*hour) * 3600
                                        + i64::from(*minute) * 60
                                        + i64::from(*second),
                                );
                            }
                        }
                    }
                }
            }
        }

        set.sort_unstable();
        set.dedup();
        if self.by_set_pos.is_empty() || set.is_empty() {
            return set;
        }
        let length = set.len() as i64;
        let mut selected: Vec<i64> = self
            .by_set_pos
            .iter()
            .filter_map(|position| {
                let position = i64::from(*position);
                let index = if position > 0 {
                    position - 1
                } else {
                    length + position
                };
                (0..length).contains(&index).then(|| set[index as usize])
            })
            .collect();
        selected.sort_unstable();
        selected.dedup();
        selected
    }

    fn advance(&mut self) {
        match self.frequency {
            Frequency::Hourly | Frequency::Minutely | Frequency::Secondly => {
                let unit = match self.frequency {
                    Frequency::Hourly => 3600,
                    Frequency::Minutely => 60,
                    _ => 1,
                };
                let next = self.period + self.interval;
                let day = (self.period * unit).div_euclid(DAY);
                let next_day = (next * unit).div_euclid(DAY);
                if next_day == day || self.day_matches(next_day, 0) {
                    self.period = next;
                    return;
                }
                let mut candidate = next;
                loop {
                    let current_day = (candidate * unit).div_euclid(DAY);
                    if self.day_matches(current_day, 0) || current_day > self.horizon_day() {
                        self.period = candidate;
                        return;
                    }
                    let into_day = candidate * unit - current_day * DAY;
                    let remaining = (DAY - into_day + unit - 1) / unit;
                    let steps = (remaining + self.interval - 1) / self.interval;
                    candidate += steps.max(1) * self.interval;
                }
            }
            Frequency::Weekly => self.period += 7 * self.interval,
            _ => self.period += self.interval,
        }
    }

    fn horizon_day(&self) -> i64 {
        self.last.div_euclid(DAY) + HORIZON_YEARS * 366
    }

    fn is_exhausted(&self) -> bool {
        let (first, _, _) = self.period_days();
        first > self.horizon_day() || civil_from_days(first).0 > LAST_YEAR
    }

    fn emit(&mut self, time: i64) -> Option<i64> {
        if self.count.is_some_and(|count| self.emitted >= count)
            || self.until.is_some_and(|until| time > until)
        {
            self.finished = true;
            return None;
        }
        self.emitted += 1;
        self.last = time;
        Some(time)
    }
}

impl Iterator for Expansion {
    type Item = i64;

    fn next(&mut self) -> Option<i64> {
        if self.finished {
            return None;
        }
        if !self.started {
            self.started = true;
            return self.emit(self.start);
        }
        loop {
            if let Some(time) = self.pending.pop_front() {
                if time <= self.last {
                    continue;
                }
                return self.emit(time);
            }
            if self.is_exhausted() {
                self.finished = true;
                return None;
            }
            let set = self.period_set();
            self.advance();
            self.pending
                .extend(set.into_iter().filter(|time| *time > self.start));
        }
    }
}

fn week_one(year: i64, week_start: u32) -> i64 {
    let first = days_from_civil(year, 1, 1);
    let offset = i64::from((weekday_from_days(first) + 7 - week_start) % 7);
    if 7 - offset >= 4 {
        first - offset
    } else {
        first - offset + 7
    }
}

fn matches_any_index(indices: &[i32], value: i64, length: i64) -> bool {
    indices.iter().any(|index| {
        let index = i64::from(*index);
        if index > 0 {
            index == value
        } else {
            length + 1 + index == value
        }
    })
}

fn weekday_from_offset(offset: u32) -> jiff::civil::Weekday {
    jiff::civil::Weekday::from_monday_zero_offset(offset as i8)
        .unwrap_or(jiff::civil::Weekday::Monday)
}
