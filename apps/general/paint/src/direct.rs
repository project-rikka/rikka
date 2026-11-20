use rikka_app::prelude::*;
use rikka_image_edit::{
    AutoColor, AutoContrast, AutoLevels, Desaturate, Equalize, FindEdges, Invert, Operation,
    Recipe, Sepia, Step,
};

use crate::operation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direct {
    AutoLevels,
    AutoContrast,
    AutoColor,
    Equalize,
    Invert,
    Desaturate(Desaturate),
    Sepia,
    FindEdges,
}

impl Direct {
    pub(crate) fn operation(self) -> Operation {
        match self {
            Direct::AutoLevels => AutoLevels::new().into(),
            Direct::AutoContrast => AutoContrast::new().into(),
            Direct::AutoColor => AutoColor::new().into(),
            Direct::Equalize => Equalize.into(),
            Direct::Invert => Invert.into(),
            Direct::Desaturate(method) => method.into(),
            Direct::Sepia => Sepia::new().into(),
            Direct::FindEdges => FindEdges.into(),
        }
    }

    pub(crate) fn recipe(self) -> Recipe {
        Recipe::new().then(Step::new(self.operation()))
    }

    pub(crate) fn name(self) -> Label {
        operation::name(&self.operation())
    }

    pub(crate) fn is_effect(self) -> bool {
        self == Direct::FindEdges
    }
}

pub(crate) fn desaturation(method: Desaturate) -> Label {
    match method {
        Desaturate::Lightness => tr!("paint.desaturate.lightness"),
        Desaturate::Luminosity => tr!("paint.desaturate.luminosity"),
        Desaturate::Average => tr!("paint.desaturate.average"),
    }
}
