use rikka_app::prelude::*;
use rikka_ui::Action;

use crate::gestures::{Direction, Swipe, SwipePhase};

const COMMITTED: f32 = 1.0;
const FULL_SWIPE: f32 = 160.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SwipeState {
    travel: f32,
}

pub(crate) fn catch_gestures<S, P>(
    content: impl View,
    swiped: impl IntoHandler<(Swipe,), (), S>,
    pinched: impl IntoHandler<(GesturePhase, f32), (), P>,
) -> Element {
    Element::with_children(
        GestureCatcher {
            swiped: Action::new(swiped),
            pinched: Action::new(pinched),
        },
        vec![content.build()],
    )
}

struct GestureCatcher {
    swiped: Action<(Swipe,)>,
    pinched: Action<(GesturePhase, f32)>,
}

fn swipe(travel: f32, phase: SwipePhase) -> Swipe {
    Swipe {
        direction: match travel > 0.0 {
            true => Direction::Back,
            false => Direction::Forward,
        },
        progress: (travel.abs() / FULL_SWIPE).clamp(0.0, COMMITTED),
        phase,
    }
}

impl Widget for GestureCatcher {
    type State = SwipeState;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, state: &mut SwipeState, event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::Gesture { gesture, .. } = event else {
            return;
        };
        match *gesture {
            Gesture::Swipe { phase, delta, .. } => {
                ctx.set_handled();
                let shown = match phase {
                    GesturePhase::Began => {
                        state.travel = 0.0;
                        return;
                    }
                    GesturePhase::Moved => {
                        state.travel += delta.x;
                        swipe(state.travel, SwipePhase::Moved)
                    }
                    GesturePhase::Ended => {
                        let shown = swipe(state.travel, SwipePhase::Committed);
                        match shown.progress >= COMMITTED {
                            true => shown,
                            false => swipe(state.travel, SwipePhase::Cancelled),
                        }
                    }
                    GesturePhase::Cancelled => swipe(state.travel, SwipePhase::Cancelled),
                };
                ctx.act(&self.swiped, (shown,));
            }
            Gesture::Pinch { phase, scale, .. } => {
                ctx.set_handled();
                ctx.act(&self.pinched, (phase, scale));
            }
            Gesture::Hold { .. } => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.swiped == other.swiped && self.pinched == other.pinched
    }

    fn debug_name(&self) -> &'static str {
        "GestureCatcher"
    }
}
