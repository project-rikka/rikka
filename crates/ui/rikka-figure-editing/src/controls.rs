use std::sync::LazyLock;

use rikka_text::installed_families;
use rikka_ui::{Label, MaterialSymbol, tr};
use rikka_widgets::{Combo, ControlSize, Segmented, combo, segmented};

use rikka_figure::TextAlign;

static FAMILIES: LazyLock<Vec<Label>> =
    LazyLock::new(|| installed_families().into_iter().map(Label::from).collect());

pub fn font_choice<S: 'static>(font: &str, choose: impl Fn(&mut S, String) + 'static) -> Combo {
    let at = FAMILIES.iter().position(|name| name.as_str() == font);
    combo(at)
        .options(FAMILIES.iter().cloned())
        .placeholder(Label::from(font))
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(move |host: &mut S, at: usize| {
            if let Some(name) = FAMILIES.get(at) {
                choose(host, name.as_str().to_owned());
            }
        })
}

pub fn align_choice<S: 'static>(
    align: TextAlign,
    choose: impl Fn(&mut S, TextAlign) + 'static,
) -> Segmented {
    let at = TextAlign::ALL
        .iter()
        .position(|candidate| *candidate == align)
        .unwrap_or_default();
    segmented(at)
        .segments(TextAlign::ALL.map(align_name))
        .glyphs(TextAlign::ALL.map(align_symbol))
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(move |host: &mut S, at: usize| {
            if let Some(align) = TextAlign::ALL.get(at) {
                choose(host, *align);
            }
        })
}

fn align_name(align: TextAlign) -> Label {
    match align {
        TextAlign::Left => tr!("figure.align.left"),
        TextAlign::Center => tr!("figure.align.center"),
        TextAlign::Right => tr!("figure.align.right"),
    }
}

fn align_symbol(align: TextAlign) -> MaterialSymbol {
    match align {
        TextAlign::Left => MaterialSymbol::FORMAT_ALIGN_LEFT,
        TextAlign::Center => MaterialSymbol::FORMAT_ALIGN_CENTER,
        TextAlign::Right => MaterialSymbol::FORMAT_ALIGN_RIGHT,
    }
}
