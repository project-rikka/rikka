#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Band {
    pub(crate) top: i32,
    pub(crate) bottom: i32,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) left: i32,
    pub(crate) right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Union,
    Subtract,
    Intersect,
}

impl Op {
    fn keeps(self, in_a: bool, in_b: bool) -> bool {
        match self {
            Op::Union => in_a || in_b,
            Op::Subtract => in_a && !in_b,
            Op::Intersect => in_a && in_b,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Edges {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl Edges {
    pub(crate) fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Bands {
    pub(crate) bands: Vec<Band>,
    pub(crate) spans: Vec<Span>,
}

impl Bands {
    pub(crate) const fn new() -> Self {
        Bands {
            bands: Vec::new(),
            spans: Vec::new(),
        }
    }

    pub(crate) fn single(edges: Edges) -> Self {
        let mut bands = Bands::default();
        bands.append(edges);
        bands
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.bands.clear();
        self.spans.clear();
    }

    pub(crate) fn spans_of(&self, band: &Band) -> &[Span] {
        &self.spans[band.start as usize..band.end as usize]
    }

    pub(crate) fn below_all(&self, edges: Edges) -> bool {
        self.bands
            .last()
            .is_none_or(|last| last.bottom <= edges.top)
    }

    pub(crate) fn append(&mut self, edges: Edges) {
        if edges.is_empty() {
            return;
        }
        let start = self.spans.len();
        self.spans.push(Span {
            left: edges.left,
            right: edges.right,
        });
        self.close_band(edges.top, edges.bottom, start);
    }

    fn close_band(&mut self, top: i32, bottom: i32, start: usize) {
        let end = self.spans.len();
        if start == end {
            return;
        }
        if let Some(last) = self.bands.last_mut()
            && last.bottom == top
            && self.spans[last.start as usize..last.end as usize] == self.spans[start..end]
        {
            last.bottom = bottom;
            self.spans.truncate(start);
            return;
        }
        self.bands.push(Band {
            top,
            bottom,
            start: index(start),
            end: index(end),
        });
    }

    pub(crate) fn combine(a: &Bands, b: &Bands, op: Op, out: &mut Bands) {
        out.clear();
        let (mut i, mut j) = (0, 0);
        let mut y = i32::MIN;
        loop {
            while a.bands.get(i).is_some_and(|band| band.bottom <= y) {
                i += 1;
            }
            while b.bands.get(j).is_some_and(|band| band.bottom <= y) {
                j += 1;
            }
            let (next_a, next_b) = (a.bands.get(i), b.bands.get(j));
            let done = match op {
                Op::Union => next_a.is_none() && next_b.is_none(),
                Op::Subtract => next_a.is_none(),
                Op::Intersect => next_a.is_none() || next_b.is_none(),
            };
            if done {
                return;
            }
            let top = [next_a, next_b]
                .into_iter()
                .flatten()
                .map(|band| band.top.max(y))
                .min()
                .unwrap_or(y);
            let active_a = next_a.filter(|band| band.top <= top);
            let active_b = next_b.filter(|band| band.top <= top);
            let bottom = [(next_a, active_a.is_some()), (next_b, active_b.is_some())]
                .into_iter()
                .filter_map(|(band, active)| {
                    band.map(|band| if active { band.bottom } else { band.top })
                })
                .min()
                .unwrap_or(i32::MAX);
            let start = out.spans.len();
            combine_spans(
                active_a.map_or(&[], |band| a.spans_of(band)),
                active_b.map_or(&[], |band| b.spans_of(band)),
                op,
                &mut out.spans,
            );
            out.close_band(top, bottom, start);
            y = bottom;
        }
    }

    pub(crate) fn band_at(&self, y: i32) -> Option<&Band> {
        let at = self.bands.partition_point(|band| band.bottom <= y);
        self.bands.get(at).filter(|band| band.top <= y)
    }

    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        self.span_containing(x, y).is_some()
    }

    pub(crate) fn span_containing(&self, x: i32, y: i32) -> Option<Edges> {
        let band = self.band_at(y)?;
        let span = span_at(self.spans_of(band), x).filter(|span| span.left <= x)?;
        Some(Edges {
            left: span.left,
            top: band.top,
            right: span.right,
            bottom: band.bottom,
        })
    }

    pub(crate) fn overlaps(&self, edges: Edges) -> bool {
        if edges.is_empty() {
            return false;
        }
        let from = self.bands.partition_point(|band| band.bottom <= edges.top);
        self.bands[from..]
            .iter()
            .take_while(|band| band.top < edges.bottom)
            .any(|band| {
                span_at(self.spans_of(band), edges.left).is_some_and(|span| span.left < edges.right)
            })
    }

    pub(crate) fn covers(&self, edges: Edges) -> bool {
        if edges.is_empty() {
            return true;
        }
        let mut at = self.bands.partition_point(|band| band.bottom <= edges.top);
        let mut y = edges.top;
        while y < edges.bottom {
            let Some(band) = self.bands.get(at) else {
                return false;
            };
            if band.top > y {
                return false;
            }
            let spans = self.spans_of(band);
            let spanned = span_at(spans, edges.left)
                .is_some_and(|span| span.left <= edges.left && span.right >= edges.right);
            if !spanned {
                return false;
            }
            y = band.bottom;
            at += 1;
        }
        true
    }

    pub(crate) fn bounds(&self) -> Option<Edges> {
        let first = self.bands.first()?;
        let last = self.bands.last()?;
        let (left, right) = self
            .bands
            .iter()
            .fold((i32::MAX, i32::MIN), |(left, right), band| {
                let spans = self.spans_of(band);
                (
                    left.min(spans[0].left),
                    right.max(spans[spans.len() - 1].right),
                )
            });
        Some(Edges {
            left,
            top: first.top,
            right,
            bottom: last.bottom,
        })
    }

    pub(crate) fn translate(&mut self, dx: i32, dy: i32) {
        for band in &mut self.bands {
            band.top = band.top.saturating_add(dy);
            band.bottom = band.bottom.saturating_add(dy);
        }
        for span in &mut self.spans {
            span.left = span.left.saturating_add(dx);
            span.right = span.right.saturating_add(dx);
        }
    }
}

fn span_at(spans: &[Span], x: i32) -> Option<&Span> {
    spans.get(spans.partition_point(|span| span.right <= x))
}

fn combine_spans(a: &[Span], b: &[Span], op: Op, out: &mut Vec<Span>) {
    let (mut i, mut j) = (0, 0);
    let (mut in_a, mut in_b) = (false, false);
    let mut open = None;
    loop {
        let (edge_a, edge_b) = (edge(a, i), edge(b, j));
        let x = match (edge_a, edge_b) {
            (None, None) => return,
            (Some(x), None) | (None, Some(x)) => x,
            (Some(p), Some(q)) => p.min(q),
        };
        if edge_a == Some(x) {
            in_a = !in_a;
            i += 1;
        }
        if edge_b == Some(x) {
            in_b = !in_b;
            j += 1;
        }
        match (open, op.keeps(in_a, in_b)) {
            (None, true) => open = Some(x),
            (Some(left), false) => {
                out.push(Span { left, right: x });
                open = None;
            }
            _ => {}
        }
    }
}

fn edge(spans: &[Span], at: usize) -> Option<i32> {
    let span = spans.get(at / 2)?;
    Some(if at.is_multiple_of(2) {
        span.left
    } else {
        span.right
    })
}

fn index(at: usize) -> u32 {
    u32::try_from(at).expect("a region holds fewer than 2^32 spans")
}
