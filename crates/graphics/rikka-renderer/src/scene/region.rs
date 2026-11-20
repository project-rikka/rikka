use rikka_types::Rect;

#[derive(Debug, Default)]
pub(crate) struct Region {
    edges: Vec<Edge>,
    active: Vec<usize>,
    spans: Vec<Span>,
    open: Vec<Run>,
    merged: Vec<Run>,
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    y: f32,
    rect: usize,
    opening: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Span {
    left: f32,
    right: f32,
}

#[derive(Clone, Copy, Debug)]
struct Run {
    span: Span,
    top: f32,
}

impl Region {
    pub(crate) fn build(&mut self, rects: &[Rect], out: &mut Vec<Rect>) {
        out.clear();
        self.edges.clear();
        self.active.clear();
        self.open.clear();
        for (index, rect) in rects.iter().enumerate() {
            if rect.is_empty() {
                continue;
            }
            self.edges.push(Edge {
                y: rect.min_y(),
                rect: index,
                opening: true,
            });
            self.edges.push(Edge {
                y: rect.max_y(),
                rect: index,
                opening: false,
            });
        }
        self.edges.sort_by(|left, right| left.y.total_cmp(&right.y));

        let mut cursor = 0;
        let mut previous = f32::NEG_INFINITY;
        while cursor < self.edges.len() {
            let y = self.edges[cursor].y;
            if self.active.is_empty() {
                self.close(previous, out);
            } else if previous < y {
                self.band(rects, previous, out);
            }
            while cursor < self.edges.len() && self.edges[cursor].y == y {
                let edge = self.edges[cursor];
                if edge.opening {
                    let left = rects[edge.rect].min_x();
                    let at = self
                        .active
                        .partition_point(|&other| rects[other].min_x() < left);
                    self.active.insert(at, edge.rect);
                } else if let Some(at) = self.active.iter().position(|&other| other == edge.rect) {
                    self.active.remove(at);
                }
                cursor += 1;
            }
            previous = y;
        }
        self.close(previous, out);
    }

    fn band(&mut self, rects: &[Rect], top: f32, out: &mut Vec<Rect>) {
        self.spans.clear();
        for &index in &self.active {
            let rect = rects[index];
            let (left, right) = (rect.min_x(), rect.max_x());
            match self.spans.last_mut() {
                Some(last) if left <= last.right => last.right = last.right.max(right),
                _ => self.spans.push(Span { left, right }),
            }
        }

        self.merged.clear();
        let (mut open, mut span) = (0, 0);
        while open < self.open.len() || span < self.spans.len() {
            match (self.open.get(open), self.spans.get(span)) {
                (Some(run), Some(target)) if run.span == *target => {
                    self.merged.push(*run);
                    open += 1;
                    span += 1;
                }
                (Some(run), Some(target)) if run.span.left < target.left => {
                    out.push(finish(*run, top));
                    open += 1;
                }
                (Some(run), Some(target)) if run.span.left > target.left => {
                    self.merged.push(Run { span: *target, top });
                    span += 1;
                }
                (Some(run), Some(target)) => {
                    out.push(finish(*run, top));
                    self.merged.push(Run { span: *target, top });
                    open += 1;
                    span += 1;
                }
                (Some(run), None) => {
                    out.push(finish(*run, top));
                    open += 1;
                }
                (None, Some(target)) => {
                    self.merged.push(Run { span: *target, top });
                    span += 1;
                }
                (None, None) => unreachable!("one bound is in range"),
            }
        }
        std::mem::swap(&mut self.open, &mut self.merged);
    }

    fn close(&mut self, bottom: f32, out: &mut Vec<Rect>) {
        out.extend(self.open.drain(..).map(|run| finish(run, bottom)));
    }
}

fn finish(run: Run, bottom: f32) -> Rect {
    Rect::new(
        run.span.left,
        run.top,
        run.span.right - run.span.left,
        bottom - run.top,
    )
}
