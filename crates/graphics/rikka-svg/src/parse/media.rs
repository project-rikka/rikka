use crate::parse::values::{Unit, length};

const FONT_SIZE: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Media {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) scheme: ColorScheme,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Bound {
    Exact,
    Least,
    Most,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Feature {
    Width(Bound, f32),
    Height(Bound, f32),
    AspectRatio(Bound, f32),
    Portrait,
    Landscape,
    Scheme(ColorScheme),
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
struct Query {
    negated: bool,
    shown: bool,
    features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct QueryList {
    queries: Vec<Query>,
}

impl QueryList {
    pub(crate) fn parse(text: &str) -> QueryList {
        QueryList {
            queries: text.split(',').map(query).collect(),
        }
    }

    pub(crate) fn matches(&self, media: &Media) -> bool {
        self.queries.iter().any(|query| {
            let holds = query.shown && query.features.iter().all(|feature| feature.holds(media));
            holds != query.negated
        })
    }

    fn asks_scheme(&self) -> bool {
        self.queries.iter().any(|query| {
            query
                .features
                .iter()
                .any(|feature| matches!(feature, Feature::Scheme(_)))
        })
    }
}

impl Feature {
    fn holds(&self, media: &Media) -> bool {
        let compare = |bound: Bound, actual: f32, wanted: f32| match bound {
            Bound::Exact => (actual - wanted).abs() < 0.5,
            Bound::Least => actual >= wanted,
            Bound::Most => actual <= wanted,
        };
        match *self {
            Feature::Width(bound, wanted) => compare(bound, media.width, wanted),
            Feature::Height(bound, wanted) => compare(bound, media.height, wanted),
            Feature::AspectRatio(bound, wanted) => {
                let ratio = media.width / media.height;
                match bound {
                    Bound::Exact => (ratio - wanted).abs() <= wanted * 1.0e-3,
                    _ => compare(bound, ratio, wanted),
                }
            }
            Feature::Portrait => media.height >= media.width,
            Feature::Landscape => media.width > media.height,
            Feature::Scheme(scheme) => media.scheme == scheme,
            Feature::Unknown => false,
        }
    }
}

fn query(text: &str) -> Query {
    let normalized = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    let mut words = normalized.as_str();
    let mut negated = false;
    if let Some(rest) = strip_keyword(words, "not") {
        negated = true;
        words = rest;
    } else if let Some(rest) = strip_keyword(words, "only") {
        words = rest;
    }
    let mut shown = true;
    let mut features = Vec::new();
    for (index, part) in split_and(words).into_iter().enumerate() {
        let part = part.trim();
        match part
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
        {
            Some(inner) => features.push(feature(inner)),
            None if index == 0 => {
                shown = matches!(part.to_ascii_lowercase().as_str(), "all" | "screen" | "");
            }
            None => features.push(Feature::Unknown),
        }
    }
    Query {
        negated,
        shown,
        features,
    }
}

fn strip_keyword<'t>(text: &'t str, keyword: &str) -> Option<&'t str> {
    let head = text.get(..keyword.len())?;
    let rest = &text[keyword.len()..];
    (head.eq_ignore_ascii_case(keyword) && rest.starts_with(char::is_whitespace))
        .then(|| rest.trim_start())
}

fn split_and(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            _ if depth == 0
                && bytes
                    .get(at..at + 5)
                    .is_some_and(|word| word.eq_ignore_ascii_case(b" and ")) =>
            {
                parts.push(&text[start..at]);
                at += 5;
                start = at;
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    parts.push(&text[start..]);
    parts
}

fn feature(text: &str) -> Feature {
    let Some((name, value)) = text.split_once(':') else {
        return Feature::Unknown;
    };
    let name = name.trim().to_ascii_lowercase();
    let value = value.trim();
    let (bound, base) = match (name.strip_prefix("min-"), name.strip_prefix("max-")) {
        (Some(base), _) => (Bound::Least, base),
        (_, Some(base)) => (Bound::Most, base),
        _ => (Bound::Exact, name.as_str()),
    };
    match base {
        "width" => pixels(value).map_or(Feature::Unknown, |wanted| Feature::Width(bound, wanted)),
        "height" => pixels(value).map_or(Feature::Unknown, |wanted| Feature::Height(bound, wanted)),
        "aspect-ratio" => ratio(value).map_or(Feature::Unknown, |wanted| {
            Feature::AspectRatio(bound, wanted)
        }),
        "orientation" if bound == Bound::Exact => match value.to_ascii_lowercase().as_str() {
            "portrait" => Feature::Portrait,
            "landscape" => Feature::Landscape,
            _ => Feature::Unknown,
        },
        "prefers-color-scheme" if bound == Bound::Exact => {
            match value.to_ascii_lowercase().as_str() {
                "light" => Feature::Scheme(ColorScheme::Light),
                "dark" => Feature::Scheme(ColorScheme::Dark),
                _ => Feature::Unknown,
            }
        }
        _ => Feature::Unknown,
    }
}

fn pixels(value: &str) -> Option<f32> {
    let given = length(value)?;
    match given.unit {
        Unit::User => Some(given.value),
        Unit::Em => Some(given.value * FONT_SIZE),
        Unit::Ex => Some(given.value * FONT_SIZE / 2.0),
        Unit::Percent => None,
    }
}

fn ratio(value: &str) -> Option<f32> {
    let (width, height) = match value.split_once('/') {
        Some((width, height)) => (
            width.trim().parse::<f32>().ok()?,
            height.trim().parse().ok()?,
        ),
        None => (value.trim().parse::<f32>().ok()?, 1.0),
    };
    let ratio = width / height;
    (ratio.is_finite() && ratio > 0.0).then_some(ratio)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MediaRules {
    pub(crate) queries: Vec<QueryList>,
    pub(crate) scheme: ColorScheme,
}

impl MediaRules {
    pub(crate) fn key(&self, width: f32, height: f32) -> Vec<bool> {
        let media = Media {
            width,
            height,
            scheme: self.scheme,
        };
        self.queries
            .iter()
            .map(|query| query.matches(&media))
            .collect()
    }

    pub(crate) fn asks_scheme(&self) -> bool {
        self.queries.iter().any(QueryList::asks_scheme)
    }
}
