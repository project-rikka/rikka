use rikka_app::prelude::*;

use crate::domain::Domain;

pub(crate) fn notice(symbol: MaterialSymbol, ink: Color, message: impl Into<Label>) -> Container {
    container(banner(message).symbol(symbol).tint(ink))
        .surface()
        .radius(Corner::Panel)
        .padding(Space::NORMAL)
}

pub(crate) fn acting_notice<M>(
    symbol: MaterialSymbol,
    ink: Color,
    message: impl Into<Label>,
    action: impl Into<Label>,
    act: impl IntoHandler<(), (), M>,
) -> Container {
    container(
        banner(message)
            .symbol(symbol)
            .tint(ink)
            .on_dismiss(action, act),
    )
    .surface()
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
}

pub(crate) fn error_notice(
    unavailable: Option<&str>,
    error: Option<&str>,
    keys: (&'static str, &'static str),
) -> Option<Container> {
    let (symbol, message) = match (unavailable, error) {
        (Some(reason), _) => (MaterialSymbol::CLOUD_OFF, tr!(keys.0, reason = reason)),
        (None, Some(reason)) => (MaterialSymbol::ERROR, tr!(keys.1, reason = reason)),
        (None, None) => return None,
    };
    Some(notice(symbol, palette().warning, message))
}

pub(crate) fn domain_notice(
    domain: &Domain,
    unavailable: &'static str,
    error: &'static str,
) -> Option<Container> {
    let unreachable = domain.unavailable().map(ToString::to_string);
    let declined = domain.error().map(ToString::to_string);
    error_notice(
        unreachable.as_deref(),
        declined.as_deref(),
        (unavailable, error),
    )
}
