use std::cell::RefCell;
use std::rc::Rc;

use rikka_ime::{Caret, PanelAnchor};
use rikka_seam::{Request, SeamClient};
use rikka_types::{Point, Rect};

use crate::{AimPin, Placement};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AimTarget {
    Caret(Rect),
    Pointer(Point),
}

#[derive(Clone, Debug)]
pub struct Aimed {
    compositor: Option<SeamClient>,
    aim: Rc<RefCell<Option<PanelAnchor>>>,
    pin: AimPin,
}

impl Aimed {
    #[must_use]
    pub(crate) fn new(compositor: Option<SeamClient>) -> Aimed {
        Aimed {
            compositor,
            aim: Rc::new(RefCell::new(None)),
            pin: AimPin::default(),
        }
    }

    #[must_use]
    pub fn pinned(self, pin: AimPin) -> Aimed {
        Aimed { pin, ..self }
    }

    pub(super) fn take_aim(&self) -> Option<String> {
        let aim = self
            .pin
            .take()
            .or_else(|| self.compositor.as_ref().and_then(ask));
        let output = aim.as_ref().map(|aim| aim.output().to_owned());
        self.aim.replace(aim);
        output
    }

    pub fn unpin(&self) {
        self.pin.clear();
    }

    #[must_use]
    pub(super) fn target_on(&self, placement: &Placement) -> Option<AimTarget> {
        let aim = self.aim.borrow();
        let aim = aim.as_ref().filter(|aim| aim.output() == placement.name)?;
        Some(match aim {
            PanelAnchor::Caret { caret, .. } => {
                AimTarget::Caret(placement.to_surface(caret.rect()))
            }
            PanelAnchor::Pointer { x, y, .. } => {
                AimTarget::Pointer(Point::new(*x as f32, *y as f32) - placement.position)
            }
        })
    }
}

fn ask(compositor: &SeamClient) -> Option<PanelAnchor> {
    compositor
        .call(Request::new(Caret::CAPABILITY, Caret::AIM).require(Caret::VERSION))
        .blocking_wait()
        .and_then(|payload| payload.decode::<Option<PanelAnchor>>())
        .inspect_err(|error| {
            tracing::warn!(
                "seam call {}/{} failed: {error}",
                Caret::CAPABILITY,
                Caret::AIM
            )
        })
        .ok()
        .flatten()
}
