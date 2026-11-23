use rikka_theme::Palette;
use rikka_ui::{Align, Color, Inset, Justify, Length, MaterialSymbol, TextColor, TextRole, Weight};

use crate::abi::violation;
use crate::ffi;

pub(crate) fn symbol(name: &str) -> MaterialSymbol {
    MaterialSymbol::from_name(name)
        .unwrap_or_else(|| violation(format!("`{name}` is not a Material Symbols name")))
}

pub(crate) fn finite(value: f64, what: &str) -> f64 {
    if !value.is_finite() {
        violation(format!("the {what} {value} is not finite"));
    }
    value
}

pub(crate) fn extent(value: f32, what: &str) -> f32 {
    if !value.is_finite() || value < 0.0 {
        violation(format!(
            "the {what} {value} is not a finite length of at least 0"
        ));
    }
    value
}

pub(crate) fn px(value: f32, what: &str) -> Length {
    Length::px(extent(value, what))
}

pub(crate) fn fraction(value: f64, what: &str) -> f32 {
    if !(0.0..=1.0).contains(&value) {
        violation(format!("the {what} {value} is not between 0 and 1"));
    }
    value as f32
}

pub(crate) fn seconds(value: f64, what: &str) -> std::time::Duration {
    std::time::Duration::try_from_secs_f64(value).unwrap_or_else(|_| {
        violation(format!(
            "the {what} {value} is not a finite number of seconds of at least 0"
        ))
    })
}

pub(crate) fn edges(top: f32, right: f32, bottom: f32, left: f32, what: &str) -> Inset {
    Inset::exact(
        extent(top, what),
        extent(right, what),
        extent(bottom, what),
        extent(left, what),
    )
}

pub(crate) fn selection(index: usize) -> Option<usize> {
    (index != ffi::NONE).then_some(index)
}

pub(crate) fn color(color: ffi::Color) -> Color {
    let channels = [color.r, color.g, color.b, color.a];
    if channels
        .iter()
        .any(|channel| !(0.0..=1.0).contains(channel))
    {
        violation(format!("{color:?} has a component outside 0 to 1"));
    }
    Color::rgba(color.r, color.g, color.b, color.a)
}

pub(crate) fn raw_color(color: Color) -> ffi::Color {
    ffi::Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

pub(crate) fn palette_color(palette: &Palette, role: ffi::ColorRole) -> ffi::Color {
    raw_color(match role {
        ffi::ColorRole::Background => palette.background,
        ffi::ColorRole::Surface => palette.surface,
        ffi::ColorRole::SurfaceVariant => palette.surface_variant,
        ffi::ColorRole::Text => palette.text,
        ffi::ColorRole::TextMuted => palette.text_muted,
        ffi::ColorRole::Accent => palette.accent,
        ffi::ColorRole::OnAccent => palette.on_accent,
        ffi::ColorRole::Selection => palette.selection,
        ffi::ColorRole::OnSelection => palette.on_selection,
        ffi::ColorRole::Danger => palette.danger,
        ffi::ColorRole::OnDanger => palette.on_danger,
        ffi::ColorRole::Warning => palette.warning,
        ffi::ColorRole::OnWarning => palette.on_warning,
        ffi::ColorRole::Success => palette.success,
        ffi::ColorRole::OnSuccess => palette.on_success,
        ffi::ColorRole::Outline => palette.outline,
        ffi::ColorRole::Overlay => palette.overlay,
    })
}

pub(crate) fn align(align: ffi::Align) -> Align {
    match align {
        ffi::Align::Start => Align::Start,
        ffi::Align::Center => Align::Center,
        ffi::Align::End => Align::End,
        ffi::Align::Stretch => Align::Stretch,
    }
}

pub(crate) fn justify(justify: ffi::Justification) -> Justify {
    match justify {
        ffi::Justification::Start => Justify::Start,
        ffi::Justification::Center => Justify::Center,
        ffi::Justification::End => Justify::End,
    }
}

pub(crate) fn role(role: ffi::TextRole) -> TextRole {
    match role {
        ffi::TextRole::Display => TextRole::Display,
        ffi::TextRole::Title => TextRole::Title,
        ffi::TextRole::Heading => TextRole::Heading,
        ffi::TextRole::Subheading => TextRole::Subheading,
        ffi::TextRole::Body => TextRole::Body,
        ffi::TextRole::Label => TextRole::Label,
        ffi::TextRole::Caption => TextRole::Caption,
    }
}

pub(crate) fn tone(tone: ffi::TextTone) -> TextColor {
    match tone {
        ffi::TextTone::Default => TextColor::Inherited,
        ffi::TextTone::Muted => TextColor::Muted,
        ffi::TextTone::Faint => TextColor::Faint,
        ffi::TextTone::Accent => TextColor::Accent,
        ffi::TextTone::Danger => TextColor::Danger,
        ffi::TextTone::Warning => TextColor::Warning,
        ffi::TextTone::Success => TextColor::Success,
    }
}

pub(crate) fn weight(weight: ffi::FontWeight) -> Weight {
    Weight(weight.into_raw() as f32)
}
