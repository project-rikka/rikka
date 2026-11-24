use rikka_renderer::Buffering;
use rikka_session::surfaces;
use rikka_theme::Theme;
use rikka_types::Color;
use rikka_ui::Inset;

use crate::error::SurfaceError;
use crate::extent::Extent;
use crate::presence_binding::PresenceBinding;
use crate::{
    Aimed, Anchor, Background, Destination, ExclusiveZone, KeyboardMode, Layer, Margin, Opacity,
    PointerMode, Positioner, Presence, Reach, SessionLock,
};

#[derive(Clone, Debug, Default)]
pub enum OutputTarget {
    #[default]
    Auto,
    All,
    Aimed(Aimed),
    Destined(Destination),
}

impl OutputTarget {
    pub fn aim(&self) -> Option<String> {
        match self {
            OutputTarget::Auto | OutputTarget::All => None,
            OutputTarget::Aimed(aimed) => aimed.take_aim(),
            OutputTarget::Destined(destination) => destination.output(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SurfaceSettings {
    pub(crate) name: Option<String>,
    pub layer: Layer,
    pub anchor: Anchor,
    pub extent: Extent,
    pub margin: Margin,
    pub exclusive: ExclusiveZone,
    pub keyboard: KeyboardMode,
    pub pointer: PointerMode,
    pub output: OutputTarget,
    pub(crate) background: Background,
    pub buffering: Buffering,
    pub(super) presence_binding: Option<PresenceBinding>,
    pub after: Vec<Presence>,
    pub reach: Option<Reach>,
    pub opacity: Option<Opacity>,
    pub drives: Option<Opacity>,
    pub fits: bool,
    pub(super) is_scrim: bool,
    pub positioner: Option<Positioner>,
    pub lock: Option<SessionLock>,
}

impl SurfaceSettings {
    pub(crate) fn new() -> Self {
        Self {
            name: None,
            layer: Layer::default(),
            anchor: Anchor::default(),
            extent: Extent::default(),
            margin: Inset::NONE,
            exclusive: ExclusiveZone::default(),
            keyboard: KeyboardMode::default(),
            pointer: PointerMode::default(),
            output: OutputTarget::default(),
            background: Background::default(),
            buffering: Buffering::default(),
            presence_binding: None,
            after: Vec::new(),
            reach: None,
            opacity: None,
            drives: None,
            fits: false,
            is_scrim: false,
            positioner: None,
            lock: None,
        }
    }

    #[must_use]
    pub fn presence(&self) -> Option<&Presence> {
        self.presence_binding
            .as_ref()
            .map(PresenceBinding::presence)
    }

    pub fn is_wanted(&self) -> bool {
        self.presence().is_none_or(Presence::is_shown)
    }

    pub fn presence_moved(&self) -> bool {
        self.presence().is_some_and(Presence::take_moved)
    }

    pub(crate) fn taken(&self) -> PointerMode {
        self.reach.as_ref().map_or(self.pointer, Reach::get)
    }

    pub fn namespace(&self, feature: &str) -> String {
        match (self.is_scrim, &self.name) {
            (true, _) => surfaces::SCRIM.to_owned(),
            (false, Some(name)) => name.clone(),
            (false, None) => feature.to_owned(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), SurfaceError> {
        match self.extent.empty_axis() {
            Some(axis) => Err(SurfaceError::Empty {
                surface: self.name.clone().unwrap_or_else(|| "a surface".to_owned()),
                axis,
            }),
            None => Ok(()),
        }
    }

    pub fn clear_color(&self) -> Color {
        match self.background {
            Background::Transparent => Color::TRANSPARENT,
            Background::Solid(color) => color,
        }
    }

    pub fn wants_alpha(&self) -> bool {
        !matches!(self.background, Background::Solid(color) if color.a >= 1.0)
    }

    pub fn blurs(&self, theme: &Theme) -> bool {
        self.is_scrim || theme.translucency().blur_radius().is_some()
    }
}
