use crate::block::{Block, Image};
use crate::reading::Reading;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Article {
    pub(crate) title: String,
    pub(crate) byline: Option<String>,
    pub(crate) site: Option<String>,
    pub(crate) published: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) direction: Direction,
    pub(crate) lead: Option<Image>,
    pub(crate) blocks: Vec<Block>,
    pub(crate) truncated: bool,
    pub(crate) reading: Reading,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Article {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn byline(&self) -> Option<&str> {
        self.byline.as_deref()
    }

    pub fn site(&self) -> Option<&str> {
        self.site.as_deref()
    }

    pub fn published(&self) -> Option<&str> {
        self.published.as_deref()
    }

    pub fn published_date(&self) -> Option<Date> {
        self.published.as_deref().and_then(Date::parse)
    }

    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn lead(&self) -> Option<&Image> {
        self.lead.as_ref()
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn reading(&self) -> Reading {
        self.reading
    }

    pub(crate) fn ideographic(&self) -> bool {
        self.language.as_deref().is_some_and(|language| {
            let primary = language.split(['-', '_']).next().unwrap_or_default();
            ["ja", "zh", "ko"]
                .iter()
                .any(|cjk| primary.eq_ignore_ascii_case(cjk))
        })
    }
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::LeftToRight => "ltr",
            Direction::RightToLeft => "rtl",
        }
    }
}

impl Date {
    fn parse(text: &str) -> Option<Date> {
        let text = text.trim();
        let digits = |range: std::ops::Range<usize>| {
            text.get(range)
                .filter(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|part| part.parse::<u16>().ok())
        };
        let separated =
            text.as_bytes().get(4) == Some(&b'-') && text.as_bytes().get(7) == Some(&b'-');
        if !separated || text.as_bytes().get(10).is_some_and(u8::is_ascii_digit) {
            return None;
        }
        let year = digits(0..4)?;
        let month = u8::try_from(digits(5..7)?).ok()?;
        let day = u8::try_from(digits(8..10)?).ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day) && year >= 1000).then_some(Date {
            year,
            month,
            day,
        })
    }
}
