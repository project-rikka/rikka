use rikka_compositor_protocols::wp_content_type_v1;
use rikka_compositor_protocols::wp_tearing_control_v1::PresentationHint;
use rikka_compositor_types::{ContentType, TearingHint};

pub(crate) fn content_type(kind: wp_content_type_v1::Type) -> ContentType {
    match kind {
        wp_content_type_v1::Type::None => ContentType::None,
        wp_content_type_v1::Type::Photo => ContentType::Photo,
        wp_content_type_v1::Type::Video => ContentType::Video,
        wp_content_type_v1::Type::Game => ContentType::Game,
    }
}

pub(crate) fn tearing_hint(hint: PresentationHint) -> TearingHint {
    match hint {
        PresentationHint::Vsync => TearingHint::Vsync,
        PresentationHint::Async => TearingHint::Async,
    }
}
