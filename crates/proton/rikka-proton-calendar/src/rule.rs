use std::fmt;
use std::str::FromStr;

use jiff::civil::Weekday;

use crate::date_value::DateValue;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Frequency {
    Secondly,
    Minutely,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Frequency::Secondly => "SECONDLY",
            Frequency::Minutely => "MINUTELY",
            Frequency::Hourly => "HOURLY",
            Frequency::Daily => "DAILY",
            Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY",
            Frequency::Yearly => "YEARLY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeekdayNum {
    pub ordinal: Option<i32>,
    pub weekday: Weekday,
}

impl WeekdayNum {
    #[must_use]
    pub fn every(weekday: Weekday) -> WeekdayNum {
        WeekdayNum {
            ordinal: None,
            weekday,
        }
    }

    #[must_use]
    pub fn nth(ordinal: i32, weekday: Weekday) -> WeekdayNum {
        WeekdayNum {
            ordinal: Some(ordinal),
            weekday,
        }
    }
}

impl fmt::Display for WeekdayNum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ordinal) = self.ordinal {
            write!(formatter, "{ordinal}")?;
        }
        formatter.write_str(format_weekday(self.weekday))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum RuleEnd {
    #[default]
    Never,
    Count(u32),
    Until(DateValue),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    pub frequency: Frequency,
    pub interval: u32,
    pub end: RuleEnd,
    pub by_second: Vec<u32>,
    pub by_minute: Vec<u32>,
    pub by_hour: Vec<u32>,
    pub by_day: Vec<WeekdayNum>,
    pub by_month_day: Vec<i32>,
    pub by_year_day: Vec<i32>,
    pub by_week_no: Vec<i32>,
    pub by_month: Vec<u32>,
    pub by_set_pos: Vec<i32>,
    pub week_start: Weekday,
    pub extensions: Vec<(String, String)>,
}

impl Rule {
    #[must_use]
    pub fn new(frequency: Frequency) -> Rule {
        Rule {
            frequency,
            interval: 1,
            end: RuleEnd::Never,
            by_second: Vec::new(),
            by_minute: Vec::new(),
            by_hour: Vec::new(),
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_year_day: Vec::new(),
            by_week_no: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
            week_start: Weekday::Monday,
            extensions: Vec::new(),
        }
    }

    #[must_use]
    pub fn every(mut self, interval: u32) -> Rule {
        self.interval = interval.max(1);
        self
    }

    #[must_use]
    pub fn count(mut self, count: u32) -> Rule {
        self.end = RuleEnd::Count(count);
        self
    }

    #[must_use]
    pub fn until(mut self, until: DateValue) -> Rule {
        self.end = RuleEnd::Until(until);
        self
    }

    #[must_use]
    pub fn on_days(mut self, days: impl IntoIterator<Item = WeekdayNum>) -> Rule {
        self.by_day = days.into_iter().collect();
        self
    }

    #[must_use]
    pub fn on_month_days(mut self, days: impl IntoIterator<Item = i32>) -> Rule {
        self.by_month_day = days.into_iter().collect();
        self
    }

    #[must_use]
    pub fn in_months(mut self, months: impl IntoIterator<Item = u32>) -> Rule {
        self.by_month = months.into_iter().collect();
        self
    }

    #[must_use]
    pub fn at_positions(mut self, positions: impl IntoIterator<Item = i32>) -> Rule {
        self.by_set_pos = positions.into_iter().collect();
        self
    }

    #[must_use]
    pub fn week_starting(mut self, weekday: Weekday) -> Rule {
        self.week_start = weekday;
        self
    }

    #[must_use]
    pub fn count_limit(&self) -> Option<u32> {
        match self.end {
            RuleEnd::Count(count) => Some(count),
            _ => None,
        }
    }

    #[must_use]
    pub fn until_value(&self) -> Option<&DateValue> {
        match &self.end {
            RuleEnd::Until(until) => Some(until),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_infinite(&self) -> bool {
        self.end == RuleEnd::Never
    }
}

impl FromStr for Rule {
    type Err = Error;

    fn from_str(text: &str) -> Result<Rule> {
        let invalid = |reason: &str| Error::InvalidRule(format!("{reason} in {text}"));
        let mut parser = RuleParser::new();
        for part in text
            .trim()
            .split(';')
            .filter(|part| !part.trim().is_empty())
        {
            let (name, value) = part
                .split_once('=')
                .ok_or_else(|| invalid("part without value"))?;
            parser
                .apply(name.trim().to_ascii_uppercase(), value.trim())
                .map_err(invalid)?;
        }
        parser.finish().map_err(invalid)
    }
}

struct RuleParser {
    rule: Rule,
    frequency: Option<Frequency>,
    count: Option<u32>,
    until: Option<DateValue>,
}

impl RuleParser {
    fn new() -> RuleParser {
        RuleParser {
            rule: Rule::new(Frequency::Daily),
            frequency: None,
            count: None,
            until: None,
        }
    }

    fn apply(&mut self, name: String, value: &str) -> std::result::Result<(), &'static str> {
        let rule = &mut self.rule;
        match name.as_str() {
            "FREQ" => self.frequency = Some(parse_frequency(value).ok_or("unknown FREQ")?),
            "INTERVAL" => {
                rule.interval = value
                    .parse()
                    .ok()
                    .filter(|interval| *interval > 0)
                    .ok_or("invalid INTERVAL")?;
            }
            "COUNT" => self.count = Some(value.parse().map_err(|_| "invalid COUNT")?),
            "UNTIL" => {
                self.until = Some(DateValue::parse(value, None).ok_or("invalid UNTIL")?);
            }
            "BYSECOND" => rule.by_second = parse_list(value, 0, 60).ok_or("invalid BYSECOND")?,
            "BYMINUTE" => rule.by_minute = parse_list(value, 0, 59).ok_or("invalid BYMINUTE")?,
            "BYHOUR" => rule.by_hour = parse_list(value, 0, 23).ok_or("invalid BYHOUR")?,
            "BYDAY" => {
                rule.by_day = value
                    .split(',')
                    .map(parse_weekday_num)
                    .collect::<Option<_>>()
                    .ok_or("invalid BYDAY")?;
            }
            "BYMONTHDAY" => {
                rule.by_month_day = parse_signed_list(value, 31).ok_or("invalid BYMONTHDAY")?;
            }
            "BYYEARDAY" => {
                rule.by_year_day = parse_signed_list(value, 366).ok_or("invalid BYYEARDAY")?;
            }
            "BYWEEKNO" => {
                rule.by_week_no = parse_signed_list(value, 53).ok_or("invalid BYWEEKNO")?;
            }
            "BYMONTH" => rule.by_month = parse_list(value, 1, 12).ok_or("invalid BYMONTH")?,
            "BYSETPOS" => {
                rule.by_set_pos = parse_signed_list(value, 366).ok_or("invalid BYSETPOS")?;
            }
            "WKST" => rule.week_start = parse_weekday(value).ok_or("invalid WKST")?,
            _ => rule.extensions.push((name, value.to_owned())),
        }
        Ok(())
    }

    fn finish(self) -> std::result::Result<Rule, &'static str> {
        let mut rule = self.rule;
        rule.frequency = self.frequency.ok_or("missing FREQ")?;
        rule.end = match (self.count, self.until) {
            (Some(_), Some(_)) => return Err("both COUNT and UNTIL"),
            (Some(count), None) => RuleEnd::Count(count),
            (None, Some(until)) => RuleEnd::Until(until),
            (None, None) => RuleEnd::Never,
        };
        for second in &mut rule.by_second {
            *second = (*second).min(59);
        }
        Ok(rule)
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FREQ={}", self.frequency.as_str())?;
        match &self.end {
            RuleEnd::Until(until) => write!(formatter, ";UNTIL={until}")?,
            RuleEnd::Count(count) => write!(formatter, ";COUNT={count}")?,
            RuleEnd::Never => {}
        }
        if self.interval > 1 {
            write!(formatter, ";INTERVAL={}", self.interval)?;
        }
        write_list(formatter, "BYSECOND", &self.by_second)?;
        write_list(formatter, "BYMINUTE", &self.by_minute)?;
        write_list(formatter, "BYHOUR", &self.by_hour)?;
        write_list(formatter, "BYDAY", &self.by_day)?;
        write_list(formatter, "BYMONTHDAY", &self.by_month_day)?;
        write_list(formatter, "BYYEARDAY", &self.by_year_day)?;
        write_list(formatter, "BYWEEKNO", &self.by_week_no)?;
        write_list(formatter, "BYMONTH", &self.by_month)?;
        write_list(formatter, "BYSETPOS", &self.by_set_pos)?;
        if self.week_start != Weekday::Monday {
            write!(formatter, ";WKST={}", format_weekday(self.week_start))?;
        }
        for (name, value) in &self.extensions {
            write!(formatter, ";{name}={value}")?;
        }
        Ok(())
    }
}

fn write_list<T: fmt::Display>(
    formatter: &mut fmt::Formatter<'_>,
    name: &str,
    values: &[T],
) -> fmt::Result {
    if values.is_empty() {
        return Ok(());
    }
    write!(formatter, ";{name}=")?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            formatter.write_str(",")?;
        }
        write!(formatter, "{value}")?;
    }
    Ok(())
}

fn parse_list(text: &str, low: u32, high: u32) -> Option<Vec<u32>> {
    text.split(',')
        .map(|part| {
            part.trim()
                .parse::<u32>()
                .ok()
                .filter(|number| (low..=high).contains(number))
        })
        .collect()
}

fn parse_signed_list(text: &str, limit: i32) -> Option<Vec<i32>> {
    text.split(',')
        .map(|part| {
            part.trim()
                .trim_start_matches('+')
                .parse::<i32>()
                .ok()
                .filter(|number| *number != 0 && number.abs() <= limit)
        })
        .collect()
}

fn parse_frequency(text: &str) -> Option<Frequency> {
    Some(match text.to_ascii_uppercase().as_str() {
        "SECONDLY" => Frequency::Secondly,
        "MINUTELY" => Frequency::Minutely,
        "HOURLY" => Frequency::Hourly,
        "DAILY" => Frequency::Daily,
        "WEEKLY" => Frequency::Weekly,
        "MONTHLY" => Frequency::Monthly,
        "YEARLY" => Frequency::Yearly,
        _ => return None,
    })
}

fn parse_weekday_num(text: &str) -> Option<WeekdayNum> {
    let text = text.trim();
    if text.len() < 2 {
        return None;
    }
    let (number, day) = text.split_at(text.len() - 2);
    let weekday = parse_weekday(day)?;
    let ordinal = match number {
        "" => None,
        number => {
            let ordinal: i32 = number.trim_start_matches('+').parse().ok()?;
            if ordinal == 0 || ordinal.abs() > 53 {
                return None;
            }
            Some(ordinal)
        }
    };
    Some(WeekdayNum { ordinal, weekday })
}

pub(crate) fn parse_weekday(text: &str) -> Option<Weekday> {
    Some(match text.trim().to_ascii_uppercase().as_str() {
        "MO" => Weekday::Monday,
        "TU" => Weekday::Tuesday,
        "WE" => Weekday::Wednesday,
        "TH" => Weekday::Thursday,
        "FR" => Weekday::Friday,
        "SA" => Weekday::Saturday,
        "SU" => Weekday::Sunday,
        _ => return None,
    })
}

pub(crate) fn format_weekday(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "MO",
        Weekday::Tuesday => "TU",
        Weekday::Wednesday => "WE",
        Weekday::Thursday => "TH",
        Weekday::Friday => "FR",
        Weekday::Saturday => "SA",
        Weekday::Sunday => "SU",
    }
}
