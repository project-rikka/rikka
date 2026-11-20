use rikka_app::prelude::*;

use super::canvas_size::CanvasSize;
use super::convert::ConvertProfile;
use super::effect::EffectDialog;
use super::layer_properties::LayerProperties;
use super::modify::Modification;
use super::resize::Resize;
use super::rotate::Rotate;
use super::{Context, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Resize,
    CanvasSize,
    Rotate,
    ConvertProfile,
    Develop(rikka_effects::Kind),
    Modify(Modification),
    LayerProperties,
}

impl Kind {
    pub(crate) fn name(self) -> Label {
        match self {
            Kind::Resize => tr!("paint.resize.title"),
            Kind::CanvasSize => tr!("paint.canvas_size.title"),
            Kind::Rotate => tr!("paint.rotate.title"),
            Kind::ConvertProfile => tr!("paint.convert.title"),
            Kind::Develop(kind) => kind.name(),
            Kind::Modify(modification) => modification.name(),
            Kind::LayerProperties => tr!("paint.layer_properties.title"),
        }
    }

    pub(crate) fn session(self, context: &Context<'_>) -> Session {
        match self {
            Kind::Resize => Session::open(Resize::from_context, context),
            Kind::CanvasSize => Session::open(CanvasSize::from_context, context),
            Kind::Rotate => Session::open(Rotate::from_context, context),
            Kind::ConvertProfile => Session::open(ConvertProfile::from_context, context),
            Kind::Develop(kind) => EffectDialog::session(kind, context),
            Kind::Modify(modification) => modification.session(context),
            Kind::LayerProperties => Session::open(LayerProperties::from_context, context),
        }
    }

    pub(crate) fn develops(self) -> bool {
        matches!(self, Kind::Develop(_))
    }
}
