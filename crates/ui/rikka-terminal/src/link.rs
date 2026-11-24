use std::ops::RangeInclusive;

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Point};

use crate::words::{self, REACH_LINES};

const SCHEMES: &[&str] = &["http", "https", "ftp", "file", "mailto"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) cells: RangeInclusive<Point>,
    pub(crate) uri: String,
    pub(crate) declared: bool,
}

impl Link {
    pub(crate) fn at<T>(term: &Term<T>, point: Point) -> Option<Link> {
        declared(term, point)
            .or_else(|| detected(term, point))
            .filter(|link| openable(&link.uri))
    }

    pub(crate) fn open(&self) {
        if let Err(error) = rikka_open::open(&self.uri) {
            tracing::warn!("failed to open link: {error}");
        }
    }
}

fn declared<T>(term: &Term<T>, point: Point) -> Option<Link> {
    let hyperlink = term.grid()[point].hyperlink()?;
    let shared = |at: Point| term.grid()[at].hyperlink().as_ref() == Some(&hyperlink);
    let first = Point::new(term.topmost_line().max(point.line - REACH_LINES), Column(0));
    let last = Point::new(
        term.bottommost_line().min(point.line + REACH_LINES),
        term.last_column(),
    );

    let mut start = point;
    while start != first {
        let previous = start.sub(term, Boundary::Grid, 1);
        if !shared(previous) {
            break;
        }
        start = previous;
    }
    let mut end = point;
    while end != last {
        let next = end.add(term, Boundary::Grid, 1);
        if !shared(next) {
            break;
        }
        end = next;
    }
    Some(Link {
        cells: start..=end,
        uri: hyperlink.uri().to_owned(),
        declared: true,
    })
}

fn detected<T>(term: &Term<T>, point: Point) -> Option<Link> {
    let (cells, uri) = words::url_at(term, point)?;
    Some(Link {
        cells,
        uri,
        declared: false,
    })
}

fn openable(uri: &str) -> bool {
    uri.split_once(':').is_some_and(|(scheme, _)| {
        SCHEMES
            .iter()
            .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    })
}
