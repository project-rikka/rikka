use std::any::Any;
use std::marker::PhantomData;

use rikka_types::Point;
use rikka_ui::{Handler, IntoHandler, set_drag_payload, take_drag_payload};

use super::strip::Strip;

const KIND: &str = "application/x-rikka-tab";

pub(crate) fn mime(group: &str) -> String {
    format!("{KIND};group={group};pid={}", std::process::id())
}

pub(crate) fn payload_handler<A: 'static, T: Any, Marker>(
    land: impl IntoHandler<(A, T), (), Marker>,
) -> Handler<(A,)> {
    let land = land.into_handler();
    Handler::relaying(move |reach, (at,)| land.call(reach, (at, take_drag_payload::<T>()?)))
}

#[must_use]
pub struct Transfer<T> {
    moves: Moves,
    carries: PhantomData<fn() -> T>,
}

#[must_use]
pub struct TransferGroup {
    group: String,
}

#[must_use]
pub struct TransferSource<T> {
    group: String,
    lift: Handler<(usize,)>,
    carries: PhantomData<fn() -> T>,
}

pub(super) struct Moves {
    group: String,
    pub(super) lift: Handler<(usize,)>,
    pub(super) land: Handler<(usize,)>,
    pub(super) tear_off: Option<Handler>,
    pub(super) tears_off_last: bool,
}

impl Transfer<()> {
    pub fn group(group: impl Into<String>) -> TransferGroup {
        TransferGroup {
            group: group.into(),
        }
    }
}

impl<T: Any> Transfer<T> {
    pub fn tear_off<Marker>(
        mut self,
        tear_off: impl IntoHandler<(T,), (), Marker>,
        tears_off_last: bool,
    ) -> Self {
        let tear_off = tear_off.into_handler();
        self.moves.tear_off = Some(Handler::relaying(move |reach, ()| {
            tear_off.call(reach, (take_drag_payload::<T>()?,))
        }));
        self.moves.tears_off_last = tears_off_last;
        self
    }

    pub(super) fn into_moves(self) -> Moves {
        self.moves
    }
}

impl TransferGroup {
    pub fn take<T: Any, Marker>(
        self,
        lift: impl IntoHandler<(usize,), Option<T>, Marker>,
    ) -> TransferSource<T> {
        let lift = lift.into_handler();
        TransferSource {
            group: self.group,
            lift: Handler::relaying(move |reach, (index,)| {
                if let Some(lifted) = lift.call(reach, (index,))? {
                    set_drag_payload(lifted);
                }
                Some(())
            }),
            carries: PhantomData,
        }
    }
}

impl<T: Any> TransferSource<T> {
    pub fn insert<Marker>(self, land: impl IntoHandler<(usize, T), (), Marker>) -> Transfer<T> {
        Transfer {
            moves: Moves {
                group: self.group,
                lift: self.lift,
                land: payload_handler(land),
                tear_off: None,
                tears_off_last: false,
            },
            carries: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Transfer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transfer")
            .field("group", &self.moves.group)
            .field("tears_off", &self.moves.tear_off.is_some())
            .field("tears_off_last", &self.moves.tears_off_last)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for TransferGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferGroup")
            .field("group", &self.group)
            .finish()
    }
}

impl<T> std::fmt::Debug for TransferSource<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferSource")
            .field("group", &self.group)
            .finish_non_exhaustive()
    }
}

pub(super) enum TransferPolicy {
    Stays,
    Accepts {
        group: String,
        land: Handler<(usize,)>,
    },
    Transfers(Moves),
}

impl TransferPolicy {
    pub(super) fn group(&self) -> Option<&str> {
        match self {
            TransferPolicy::Stays => None,
            TransferPolicy::Accepts { group, .. }
            | TransferPolicy::Transfers(Moves { group, .. }) => Some(group),
        }
    }

    pub(super) fn land(&self) -> Option<&Handler<(usize,)>> {
        match self {
            TransferPolicy::Stays => None,
            TransferPolicy::Accepts { land, .. }
            | TransferPolicy::Transfers(Moves { land, .. }) => Some(land),
        }
    }

    pub(super) fn transfer(&self) -> Option<&Moves> {
        match self {
            TransferPolicy::Transfers(moves) => Some(moves),
            TransferPolicy::Stays | TransferPolicy::Accepts { .. } => None,
        }
    }

    pub(super) fn tears_off_last(&self) -> bool {
        self.transfer().is_some_and(|moves| moves.tears_off_last)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PendingDrag {
    pub(super) index: usize,
    pub(super) origin: Point,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Dragging {
    pub(super) index: usize,
    pub(super) lifted: bool,
    pub(super) follows: bool,
    pub(super) grip: Point,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Places {
    pub(super) dragged: Option<usize>,
    pub(super) landing: Option<f32>,
}

impl Places {
    pub(super) fn is_at_rest(&self) -> bool {
        self.dragged.is_none() && self.landing.is_none()
    }

    pub(super) fn slot_position(&self, index: usize) -> f32 {
        let base = match self.dragged {
            Some(dragged) if index > dragged => index - 1,
            _ => index,
        } as f32;
        match self.landing {
            Some(landing) => base + (base + 1.0 - landing).clamp(0.0, 1.0),
            None => base,
        }
    }
}

pub(super) fn landing_at(strip: &Strip, point: Point, slots: usize) -> usize {
    let width = strip.width();
    if width <= 0.0 || slots == 0 {
        return 0;
    }
    let travelled = point.x - strip.lane().x() + strip.offset();
    let slot = (travelled / width).round().max(0.0) as usize;
    slot.min(slots - 1)
}
