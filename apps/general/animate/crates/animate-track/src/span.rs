#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Body,
    Start,
    End,
}

impl Span {
    #[must_use]
    pub fn dragged(
        self,
        edge: Edge,
        anchor: f32,
        at: f32,
        snap: impl Fn(i64) -> Option<i64>,
    ) -> Self {
        let by = (at - anchor).round() as i64;
        let shifted = |frame: u32| i64::from(frame) + by;
        let snapped = |frame: i64| frame + snap(frame).unwrap_or(0);
        match edge {
            Edge::Body => {
                let (start, end) = (shifted(self.start), self.end.map(shifted));
                let nudge = snap(start).or_else(|| end.and_then(&snap)).unwrap_or(0);
                Self {
                    start: frame(start + nudge),
                    end: end.map(|end| frame(end + nudge)),
                }
            }
            Edge::Start => Self {
                start: frame(snapped(shifted(self.start))),
                ..self
            },
            Edge::End => Self {
                end: Some(frame(snapped(self.end.map_or(at.round() as i64, shifted)))),
                ..self
            },
        }
    }
}

fn frame(at: i64) -> u32 {
    u32::try_from(at.max(0)).unwrap_or(u32::MAX)
}
