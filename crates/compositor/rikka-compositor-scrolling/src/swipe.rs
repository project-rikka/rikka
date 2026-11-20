use std::time::Duration;

use crate::fit::padded;
use crate::strip::{Hold, Strip};
use crate::tracker::Tracker;

const TRAVEL_PER_SPAN: f64 = 1200.0;

#[derive(Debug, Clone)]
pub(crate) struct Swipe {
    pub(crate) base: f64,
    tracker: Tracker,
}

#[derive(Debug, Clone, Copy)]
struct Snap {
    view: f64,
    column: usize,
}

#[derive(Debug, Clone, Copy)]
struct SnapPoint {
    x: f64,
    width: f64,
    padding: f64,
    left: f64,
    right: f64,
}

impl<W: Copy + PartialEq> Strip<W> {
    pub fn swipe_begin(&mut self) -> bool {
        if self.columns.is_empty() || matches!(self.hold, Hold::Resizing) {
            return false;
        }
        self.hold = Hold::Swiping(Swipe {
            base: self.offset,
            tracker: Tracker::default(),
        });
        true
    }

    #[must_use]
    pub fn swiping(&self) -> bool {
        matches!(self.hold, Hold::Swiping(_))
    }

    pub fn swipe_update(&mut self, dx: f64, time: Duration) {
        let scale = self.swipe_scale();
        let Hold::Swiping(swipe) = &mut self.hold else {
            return;
        };
        swipe.tracker.push(dx, time);
        self.offset = swipe.base - swipe.tracker.travel() * scale;
    }

    pub fn swipe_end(&mut self, time: Duration) -> f64 {
        let mut swipe = match std::mem::take(&mut self.hold) {
            Hold::Swiping(swipe) => swipe,
            hold => {
                self.hold = hold;
                return 0.0;
            }
        };
        let scale = self.swipe_scale();
        swipe.tracker.push(0.0, time);
        let speed = -swipe.tracker.velocity() * scale;
        self.offset = swipe.base - swipe.tracker.travel() * scale;
        if self.columns.is_empty() {
            return speed;
        }
        let current = self.view();
        let aimed = self.x_of(self.active) + swipe.base - swipe.tracker.projected() * scale;
        let snap_points = self.snap_points();
        let snap = snapped(&snap_points, aimed);
        let span = self.frame.span();
        let mut chosen = snap.column;
        if aimed >= current {
            for (index, reach) in snap_points.iter().enumerate().skip(chosen + 1) {
                if snap.view + span < reach.x + reach.width + reach.padding {
                    break;
                }
                chosen = index;
            }
        } else {
            for (index, reach) in snap_points.iter().enumerate().take(chosen).rev() {
                if reach.x - reach.padding < snap.view {
                    break;
                }
                chosen = index;
            }
        }
        match chosen == self.active {
            true => self.set_view(snap.view),
            false => self.reseat(chosen, snap.view),
        }
        speed
    }

    fn swipe_scale(&self) -> f64 {
        self.frame.span() / TRAVEL_PER_SPAN
    }

    fn snap_points(&self) -> Vec<SnapPoint> {
        let span = self.frame.span();
        let outer = self.frame.outer();
        self.columns
            .iter()
            .zip(self.xs())
            .map(|(column, x)| {
                let x = f64::from(x);
                let width = f64::from(column.extent(&self.frame));
                match column.pinned(&self.frame, x) {
                    Some(view) => SnapPoint {
                        x,
                        width,
                        padding: 0.0,
                        left: view,
                        right: view,
                    },
                    None => {
                        let padding = padded(span, width, outer);
                        SnapPoint {
                            x,
                            width,
                            padding,
                            left: x - padding,
                            right: x + width + padding - span,
                        }
                    }
                }
            })
            .collect()
    }
}

fn snapped(snap_points: &[SnapPoint], aimed: f64) -> Snap {
    let leftmost = snap_points[0].left;
    let last = snap_points.len() - 1;
    let rightmost = snap_points[last].right;
    let mut snaps = vec![
        Snap {
            view: leftmost,
            column: 0,
        },
        Snap {
            view: rightmost,
            column: last,
        },
    ];
    for (column, reach) in snap_points.iter().enumerate() {
        for view in [reach.left, reach.right] {
            if leftmost < view && view < rightmost {
                snaps.push(Snap { view, column });
            }
        }
    }
    snaps
        .into_iter()
        .min_by(|one, other| {
            (one.view - aimed)
                .abs()
                .total_cmp(&(other.view - aimed).abs())
        })
        .unwrap_or(Snap {
            view: leftmost,
            column: 0,
        })
}
