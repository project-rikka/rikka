use rikka_wayland_client::zwlr_layer_shell_v1;
use rikka_wayland_client::zwlr_layer_surface_v1::{Anchor as LayerAnchor, KeyboardInteractivity};

use shell_kit::{Anchor, ExclusiveZone, KeyboardMode, Layer};

pub(crate) fn layer(layer: Layer) -> zwlr_layer_shell_v1::Layer {
    match layer {
        Layer::Background => zwlr_layer_shell_v1::Layer::Background,
        Layer::Bottom => zwlr_layer_shell_v1::Layer::Bottom,
        Layer::Top => zwlr_layer_shell_v1::Layer::Top,
        Layer::Overlay => zwlr_layer_shell_v1::Layer::Overlay,
    }
}

pub(crate) fn anchor(anchor: Anchor) -> LayerAnchor {
    let mut out = LayerAnchor::empty();
    if anchor.contains(Anchor::TOP) {
        out |= LayerAnchor::TOP;
    }
    if anchor.contains(Anchor::BOTTOM) {
        out |= LayerAnchor::BOTTOM;
    }
    if anchor.contains(Anchor::LEFT) {
        out |= LayerAnchor::LEFT;
    }
    if anchor.contains(Anchor::RIGHT) {
        out |= LayerAnchor::RIGHT;
    }
    out
}

pub(crate) fn exclusive_zone(zone: ExclusiveZone) -> i32 {
    match zone {
        ExclusiveZone::None => 0,
        ExclusiveZone::Ignore => -1,
        ExclusiveZone::Fixed(px) => px.round() as i32,
    }
}

pub(crate) fn keyboard_mode(mode: KeyboardMode) -> KeyboardInteractivity {
    match mode {
        KeyboardMode::None => KeyboardInteractivity::None,
        KeyboardMode::OnDemand => KeyboardInteractivity::OnDemand,
        KeyboardMode::Exclusive => KeyboardInteractivity::Exclusive,
    }
}
