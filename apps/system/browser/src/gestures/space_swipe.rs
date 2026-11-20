use rikka_app::prelude::*;
use rikka_ui::Action;

const DECIDED: f32 = 16.0;
const STEPS_AT: f32 = 96.0;
const LEVEL: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SwipeTracker {
    travel: Point,
    finished: bool,
}

impl SwipeTracker {
    pub(crate) fn begin(&mut self) {
        *self = SwipeTracker::default();
    }

    pub(crate) fn moved(&mut self, delta: Point) -> Option<isize> {
        if self.finished {
            return None;
        }
        self.travel = Point::new(self.travel.x + delta.x, self.travel.y + delta.y);
        let (across, down) = (self.travel.x.abs(), self.travel.y.abs());
        if across.hypot(down) >= DECIDED && across < down * LEVEL {
            self.finished = true;
            return None;
        }
        if across < STEPS_AT {
            return None;
        }
        self.finished = true;
        Some(match self.travel.x > 0.0 {
            true => -1,
            false => 1,
        })
    }
}

pub(crate) fn space_swipe<M>(
    content: impl View,
    step: impl IntoHandler<(isize,), (), M>,
) -> Element {
    Element::with_children(
        SpaceSwipe {
            step: Action::new(step),
        },
        vec![content.build()],
    )
}

struct SpaceSwipe {
    step: Action<(isize,)>,
}

impl Widget for SpaceSwipe {
    type State = SwipeTracker;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::HUG, Length::FILL)
    }

    fn event(&self, sweep: &mut SwipeTracker, event: &Event, ctx: &mut EventCtx<'_>) {
        let moved = match *event {
            Event::Scroll {
                delta,
                source: ScrollSource::Continuous(phase),
                ..
            } => match phase {
                ScrollPhase::Began => {
                    sweep.begin();
                    sweep.moved(delta.content())
                }
                ScrollPhase::Moved => sweep.moved(delta.content()),
                ScrollPhase::Ended => None,
            },
            Event::Gesture {
                gesture: Gesture::Swipe { phase, delta, .. },
                ..
            } => match phase {
                GesturePhase::Began => {
                    sweep.begin();
                    None
                }
                GesturePhase::Moved => sweep.moved(delta),
                GesturePhase::Ended | GesturePhase::Cancelled => None,
            },
            _ => return,
        };
        ctx.set_handled();
        if let Some(by) = moved {
            ctx.act(&self.step, (by,));
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.step == other.step
    }

    fn debug_name(&self) -> &'static str {
        "SpaceSwipe"
    }
}
