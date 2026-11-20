use paint_document::{Edit, Modify};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled};

use super::form::{self, Scale, Unit};
use super::{Context, Dialog, Kind, Outcome, Session, tune};

const REACH: Scale = Scale::new(1.0..=500.0)
    .slide(1.0..=100.0)
    .unit(Unit::Pixels);
const BORDER: Scale = Scale::new(1.0..=200.0)
    .slide(1.0..=100.0)
    .unit(Unit::Pixels);
const FEATHER: Scale = Scale::new(0.1..=1000.0)
    .slide(0.1..=250.0)
    .decimals(1)
    .unit(Unit::Pixels);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modification {
    Expand,
    Contract,
    Feather,
    Border,
    Smooth,
}

impl Modification {
    pub(crate) fn name(self) -> Label {
        match self {
            Modification::Expand => tr!("paint.modify.expand"),
            Modification::Contract => tr!("paint.modify.contract"),
            Modification::Feather => tr!("paint.modify.feather"),
            Modification::Border => tr!("paint.modify.border"),
            Modification::Smooth => tr!("paint.modify.smooth"),
        }
    }

    pub(crate) fn session(self, context: &Context<'_>) -> Session {
        Session::holding(
            ModifySelection::new(self),
            move |_: &Context<'_>| ModifySelection::new(self),
            context,
        )
    }

    fn amount_label(self) -> Label {
        match self {
            Modification::Expand => tr!("paint.modify.expand_by"),
            Modification::Contract => tr!("paint.modify.contract_by"),
            Modification::Feather => tr!("paint.modify.feather_radius"),
            Modification::Border => tr!("paint.modify.width"),
            Modification::Smooth => tr!("paint.modify.sample_radius"),
        }
    }

    fn scale(self) -> &'static Scale {
        match self {
            Modification::Expand | Modification::Contract | Modification::Smooth => &REACH,
            Modification::Feather => &FEATHER,
            Modification::Border => &BORDER,
        }
    }

    fn first_amount(self) -> f32 {
        match self {
            Modification::Expand | Modification::Contract | Modification::Smooth => 2.0,
            Modification::Feather => 5.0,
            Modification::Border => 4.0,
        }
    }

    fn modify(self, amount: f32) -> Modify {
        let pixels = amount.round().max(1.0) as u32;
        match self {
            Modification::Expand => Modify::Expand(pixels),
            Modification::Contract => Modify::Contract(pixels),
            Modification::Feather => Modify::Feather(amount),
            Modification::Border => Modify::Border(pixels),
            Modification::Smooth => Modify::Smooth(pixels),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ModifySelection {
    modification: Modification,
    amount: f32,
}

impl ModifySelection {
    pub(crate) fn new(modification: Modification) -> ModifySelection {
        ModifySelection {
            modification,
            amount: modification.first_amount(),
        }
    }
}

impl Dialog for ModifySelection {
    fn title(&self) -> Label {
        Kind::Modify(self.modification).name()
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        form_sheet(vec![labelled(
            self.modification.amount_label(),
            form::sliding(
                self.amount,
                self.modification.scale(),
                tune(|dialog: &mut ModifySelection, amount: f32| dialog.amount = amount),
            ),
        )])
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        let amount = self.modification.scale().clamp(self.amount);
        Some(Outcome::Edit(Edit::ModifySelection(
            self.modification.modify(amount),
        )))
    }
}
