use std::ops::Range;

use rikka_image_edit::{Flip, Mask, QuarterTurn};

use crate::area::Area;
use crate::combine::Combine;
use crate::error::{Error, Result};

const HALF: u8 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub coverage: u8,
}

impl Span {
    pub const FULL: u8 = u8::MAX;

    #[must_use]
    pub const fn full(start: u32, end: u32) -> Span {
        Span {
            start,
            end,
            coverage: Span::FULL,
        }
    }

    #[must_use]
    pub const fn is_marked(self) -> bool {
        self.coverage >= HALF
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Region {
    top: u32,
    starts: Vec<usize>,
    spans: Vec<Span>,
    extent: Area,
    bounds: Option<Area>,
}

impl Region {
    #[must_use]
    pub fn rectangle(area: Area) -> Region {
        if area.is_empty() {
            return Region::default();
        }
        Region {
            top: area.top,
            starts: (0..=area.height as usize).collect(),
            spans: vec![Span::full(area.left, area.right()); area.height as usize],
            extent: area,
            bounds: Some(area),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    #[must_use]
    pub fn bounds(&self) -> Option<Area> {
        self.bounds
    }

    #[must_use]
    pub fn extent(&self) -> Option<Area> {
        (!self.is_empty()).then_some(self.extent)
    }

    #[must_use]
    pub fn rectangular(&self) -> Option<Area> {
        let extent = self.extent()?;
        let whole = Span::full(extent.left, extent.right());
        (self.spans.len() == extent.height as usize && self.spans.iter().all(|span| *span == whole))
            .then_some(extent)
    }

    #[must_use]
    pub fn row(&self, y: u32) -> &[Span] {
        match self.lines().contains(&y) {
            true => {
                let index = (y - self.top) as usize;
                &self.spans[self.starts[index]..self.starts[index + 1]]
            }
            false => &[],
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = (u32, &[Span])> {
        self.lines().map(|y| (y, self.row(y)))
    }

    #[must_use]
    pub fn coverage(&self, x: u32, y: u32) -> u8 {
        let row = self.row(y);
        let at = row.partition_point(|span| span.end <= x);
        row.get(at)
            .filter(|span| span.start <= x)
            .map_or(0, |span| span.coverage)
    }

    pub fn mask(&self, width: u32, height: u32) -> Result<Mask> {
        Mask::coverage(width, height, self.coverages(width, height)).map_err(Error::from)
    }

    #[must_use]
    pub fn encloses(&self, area: Area) -> bool {
        (area.top..area.top + area.height).all(|y| {
            let row = self.row(y);
            let at = row.partition_point(|span| span.end <= area.left);
            row.get(at).is_some_and(|span| {
                span.coverage == Span::FULL && span.start <= area.left && span.end >= area.right()
            })
        })
    }

    #[must_use]
    pub fn combine(&self, combine: Combine, other: Region) -> Region {
        match combine {
            Combine::Replace => other,
            _ => merged(self, &other, |existing_coverage, added| {
                combine.cover(existing_coverage, added)
            }),
        }
    }

    #[must_use]
    pub fn clipped(&self, area: Area) -> Region {
        if self.extent().is_none_or(|extent| inside(extent, area)) {
            return self.clone();
        }
        merged(self, &Region::rectangle(area), u8::min)
    }

    #[must_use]
    pub fn inverted(&self, canvas: Area) -> Region {
        Region::rectangle(canvas).combine(Combine::Exclude, self.clone())
    }

    #[must_use]
    pub fn footprint(&self) -> Region {
        let mut builder = Builder::new(self.top);
        for (_, row) in self.rows() {
            for span in row.iter().filter(|span| span.is_marked()) {
                builder.push(span.start, span.end, Span::FULL);
            }
            builder.end_row();
        }
        builder.finish()
    }

    pub(crate) fn coverages(&self, width: u32, height: u32) -> Vec<u8> {
        let mut values = vec![0u8; width as usize * height as usize];
        for (y, spans) in self.rows().take_while(|(y, _)| *y < height) {
            let row = y as usize * width as usize;
            for span in spans {
                let (from, to) = (span.start.min(width) as usize, span.end.min(width) as usize);
                values[row + from..row + to].fill(span.coverage);
            }
        }
        values
    }

    pub(crate) fn lines(&self) -> Range<u32> {
        match self.is_empty() {
            true => 0..0,
            false => self.top..self.top + self.starts.len() as u32 - 1,
        }
    }

    pub(crate) fn held_bytes(&self) -> u64 {
        (self.spans.len() * size_of::<Span>() + self.starts.len() * size_of::<usize>()) as u64
    }

    pub(crate) fn translated(&self, dx: i64, dy: i64, canvas: Area) -> Region {
        let lines = self.lines();
        let top = (i64::from(lines.start) + dy).max(i64::from(canvas.top));
        let bottom = (i64::from(lines.end) + dy).min(i64::from(canvas.top + canvas.height));
        if top >= bottom {
            return Region::default();
        }
        let (left, right) = (i64::from(canvas.left), i64::from(canvas.right()));
        let mut builder = Builder::new(top as u32);
        for y in top..bottom {
            for span in self.row((y - dy) as u32) {
                let start = (i64::from(span.start) + dx).max(left);
                let end = (i64::from(span.end) + dx).min(right);
                if start < end {
                    builder.push(start as u32, end as u32, span.coverage);
                }
            }
            builder.end_row();
        }
        builder.finish()
    }

    pub(crate) fn flipped(&self, flip: Flip, canvas: Area) -> Region {
        let lines = self.lines();
        if lines.is_empty() {
            return Region::default();
        }
        let (width, height) = (canvas.width, canvas.height);
        match flip {
            Flip::Horizontal => {
                let mut builder = Builder::new(lines.start);
                for y in lines {
                    for span in self.row(y).iter().rev() {
                        builder.push(width - span.end, width - span.start, span.coverage);
                    }
                    builder.end_row();
                }
                builder.finish()
            }
            Flip::Vertical => {
                let mut builder = Builder::new(height - lines.end);
                for y in (height - lines.end)..(height - lines.start) {
                    for span in self.row(height - 1 - y) {
                        builder.push(span.start, span.end, span.coverage);
                    }
                    builder.end_row();
                }
                builder.finish()
            }
        }
    }

    pub(crate) fn turned(&self, turn: QuarterTurn, canvas: Area) -> Region {
        let Some(extent) = self.extent() else {
            return Region::default();
        };
        let (width, height) = (canvas.width, canvas.height);
        let top = match turn {
            QuarterTurn::Half => {
                return self
                    .flipped(Flip::Horizontal, canvas)
                    .flipped(Flip::Vertical, canvas);
            }
            QuarterTurn::Clockwise => extent.left,
            QuarterTurn::CounterClockwise => width - extent.right(),
        };
        let mut rows = vec![Vec::new(); extent.width as usize];
        for (span, from, to) in self.runs() {
            let (columns, lines) = match turn {
                QuarterTurn::Clockwise => (height - to..height - from, span.start..span.end),
                _ => (from..to, width - span.end..width - span.start),
            };
            for y in lines {
                rows[(y - top) as usize].push(Span {
                    start: columns.start,
                    end: columns.end,
                    coverage: span.coverage,
                });
            }
        }
        let mut builder = Builder::new(top);
        for mut row in rows {
            row.sort_unstable_by_key(|span| span.start);
            for span in row {
                builder.push(span.start, span.end, span.coverage);
            }
            builder.end_row();
        }
        builder.finish()
    }

    fn runs(&self) -> Vec<(Span, u32, u32)> {
        let lines = self.lines();
        let mut open: Vec<(Span, u32)> = Vec::new();
        let mut done = Vec::new();
        for y in lines.start..=lines.end {
            let row = self.row(y);
            let mut next = Vec::with_capacity(row.len());
            let mut at = 0;
            for span in row {
                while at < open.len() && open[at].0.start < span.start {
                    done.push((open[at].0, open[at].1, y));
                    at += 1;
                }
                match open.get(at) {
                    Some(previous) if previous.0 == *span => {
                        next.push(*previous);
                        at += 1;
                    }
                    _ => next.push((*span, y)),
                }
            }
            done.extend(open[at..].iter().map(|(span, from)| (*span, *from, y)));
            open = next;
        }
        done
    }
}

fn inside(inner: Area, outer: Area) -> bool {
    inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right() <= outer.right()
        && inner.top + inner.height <= outer.top + outer.height
}

pub(crate) fn merge_row(
    current: &[Span],
    added: &[Span],
    cover: impl Fn(u8, u8) -> u8,
    mut emit: impl FnMut(u32, u32, u8),
) {
    let mut x = match (current.first(), added.first()) {
        (None, None) => return,
        (Some(one), None) | (None, Some(one)) => one.start,
        (Some(one), Some(other)) => one.start.min(other.start),
    };
    let (mut first, mut second) = (0, 0);
    let mut pending: Option<(u32, u32, u8)> = None;
    loop {
        while current.get(first).is_some_and(|span| span.end <= x) {
            first += 1;
        }
        while added.get(second).is_some_and(|span| span.end <= x) {
            second += 1;
        }
        let at = |spans: &[Span], index: usize| match spans.get(index) {
            None => (0, None),
            Some(span) if span.start <= x => (span.coverage, Some(span.end)),
            Some(span) => (0, Some(span.start)),
        };
        let (one, one_next) = at(current, first);
        let (other, other_next) = at(added, second);
        let next = match (one_next, other_next) {
            (None, None) => break,
            (Some(next), None) | (None, Some(next)) => next,
            (Some(one), Some(other)) => one.min(other),
        };
        let coverage = cover(one, other);
        pending = match pending {
            Some((start, end, run_coverage)) if end == x && run_coverage == coverage => {
                Some((start, next, run_coverage))
            }
            Some((start, end, run_coverage)) => {
                emit(start, end, run_coverage);
                (coverage > 0).then_some((x, next, coverage))
            }
            None => (coverage > 0).then_some((x, next, coverage)),
        };
        x = next;
    }
    if let Some((start, end, coverage)) = pending {
        emit(start, end, coverage);
    }
}

fn merged(current: &Region, added: &Region, cover: impl Fn(u8, u8) -> u8) -> Region {
    let (one, other) = (current.lines(), added.lines());
    let lines = match (cover(Span::FULL, 0) > 0, cover(0, Span::FULL) > 0) {
        (false, false) => one.start.max(other.start)..one.end.min(other.end),
        (true, false) => one,
        (false, true) => other,
        (true, true) if one.is_empty() => other,
        (true, true) if other.is_empty() => one,
        (true, true) => one.start.min(other.start)..one.end.max(other.end),
    };
    if lines.is_empty() {
        return Region::default();
    }
    let mut builder = Builder::new(lines.start);
    for y in lines {
        merge_row(
            current.row(y),
            added.row(y),
            &cover,
            |start, end, coverage| {
                builder.push(start, end, coverage);
            },
        );
        builder.end_row();
    }
    builder.finish()
}

pub(crate) struct Builder {
    top: u32,
    starts: Vec<usize>,
    spans: Vec<Span>,
}

impl Builder {
    pub(crate) fn new(top: u32) -> Builder {
        Builder {
            top,
            starts: vec![0],
            spans: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, start: u32, end: u32, coverage: u8) {
        if coverage == 0 || start >= end {
            return;
        }
        let in_row = self.spans.len() > self.starts[self.starts.len() - 1];
        match self.spans.last_mut() {
            Some(last) if in_row && last.end == start && last.coverage == coverage => {
                last.end = end;
            }
            _ => self.spans.push(Span {
                start,
                end,
                coverage,
            }),
        }
    }

    pub(crate) fn end_row(&mut self) {
        self.starts.push(self.spans.len());
    }

    pub(crate) fn finish(self) -> Region {
        let filled = |row: &usize| self.starts[*row] < self.starts[*row + 1];
        let rows = self.starts.len() - 1;
        let (Some(first), Some(last)) = ((0..rows).find(filled), (0..rows).rfind(filled)) else {
            return Region::default();
        };
        let starts = self.starts[first..=last + 1].to_vec();
        let top = self.top + first as u32;
        let (mut left, mut right) = (u32::MAX, 0);
        let mut marked: Option<(u32, u32, u32, u32)> = None;
        for (index, row) in starts.windows(2).enumerate() {
            let spans = &self.spans[row[0]..row[1]];
            let (Some(first_span), Some(last_span)) = (spans.first(), spans.last()) else {
                continue;
            };
            left = left.min(first_span.start);
            right = right.max(last_span.end);
            let y = top + index as u32;
            let mut marked_spans = spans.iter().filter(|span| span.is_marked());
            if let Some(first_marked) = marked_spans.next() {
                let last_marked = marked_spans.next_back().unwrap_or(first_marked);
                marked = Some(match marked {
                    None => (first_marked.start, last_marked.end, y, y + 1),
                    Some((from, to, upper, _)) => (
                        from.min(first_marked.start),
                        to.max(last_marked.end),
                        upper,
                        y + 1,
                    ),
                });
            }
        }
        let extent = Area::new(left, top, right - left, (last - first + 1) as u32);
        let bounds =
            marked.map(|(from, to, upper, lower)| Area::new(from, upper, to - from, lower - upper));
        Region {
            top,
            starts,
            spans: self.spans,
            extent,
            bounds,
        }
    }
}
