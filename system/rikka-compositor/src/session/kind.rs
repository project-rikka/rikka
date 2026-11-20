use rikka_settings::Layers;

use super::bus::PrivateBus;

#[derive(Debug)]
pub(crate) enum SessionKind {
    Login,
    Nested(PrivateBus),
    Headless(Option<PrivateBus>),
}

impl SessionKind {
    pub(crate) fn is_login(&self) -> bool {
        matches!(self, SessionKind::Login)
    }

    pub(crate) fn private_bus(&self) -> Option<&PrivateBus> {
        match self {
            SessionKind::Nested(bus) | SessionKind::Headless(Some(bus)) => Some(bus),
            SessionKind::Login | SessionKind::Headless(None) => None,
        }
    }

    pub(crate) fn settings_layers(&self) -> rikka_settings::Result<Layers> {
        let layers = Layers::resolve()?;
        Ok(match self {
            SessionKind::Nested(_) => layers.volatile(),
            SessionKind::Login | SessionKind::Headless(_) => layers,
        })
    }
}
