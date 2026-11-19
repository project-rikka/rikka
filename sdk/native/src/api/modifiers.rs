use rikka_ui::{
    Anchor, AnchorEdge, Edge, Family, Inset, Length, Point, Space, TextAlign, TextStyled,
    Transition, fade, fade_scale, pop, slide,
};
use rikka_widgets::{ControlSize, ImageFit};

use crate::abi::violation;
use crate::api::elements::check_state;
use crate::ffi;
use crate::values::{
    align, color, edges, extent, finite, px, role, selection as selected_index, symbol, weight,
};
use crate::view::node::Kind;
use crate::view::{View, key_of};

fn size_class(size: ffi::SizeClass) -> ControlSize {
    match size {
        ffi::SizeClass::Small => ControlSize::Small,
        ffi::SizeClass::Medium => ControlSize::Medium,
        ffi::SizeClass::Large => ControlSize::Large,
    }
}

pub(crate) fn width(view: &mut View, value: f32) {
    view.current_element().common.style.width = Some(px(value, "width"));
}

pub(crate) fn height(view: &mut View, value: f32) {
    view.current_element().common.style.height = Some(px(value, "height"));
}

pub(crate) fn min_width(view: &mut View, value: f32) {
    view.current_element().common.style.min_width = Some(px(value, "width"));
}

pub(crate) fn min_height(view: &mut View, value: f32) {
    view.current_element().common.style.min_height = Some(px(value, "height"));
}

pub(crate) fn max_width(view: &mut View, value: f32) {
    view.current_element().common.style.max_width = Some(px(value, "width"));
}

pub(crate) fn max_height(view: &mut View, value: f32) {
    view.current_element().common.style.max_height = Some(px(value, "height"));
}

pub(crate) fn fill_width(view: &mut View) {
    view.current_element().common.style.width = Some(Length::FILL);
}

pub(crate) fn fill_height(view: &mut View) {
    view.current_element().common.style.height = Some(Length::FILL);
}

pub(crate) fn grow(view: &mut View, factor: f32) {
    view.current_element().common.style.grow = Some(extent(factor, "grow factor"));
}

pub(crate) fn shrink(view: &mut View, factor: f32) {
    view.current_element().common.style.shrink = Some(extent(factor, "shrink factor"));
}

pub(crate) fn align_self(view: &mut View, value: ffi::Align) {
    view.current_element().common.style.align_self = Some(align(value));
}

pub(crate) fn margin(view: &mut View, value: f32) {
    view.current_element().common.style.margin = Some(Inset::all(extent(value, "margin")));
}

pub(crate) fn margin_edges(view: &mut View, top: f32, right: f32, bottom: f32, left: f32) {
    view.current_element().common.style.margin = Some(edges(top, right, bottom, left, "margin"));
}

pub(crate) fn position(view: &mut View, x: f32, y: f32) {
    if !(x.is_finite() && y.is_finite()) {
        violation(format!("({x}, {y}) is not a finite position"));
    }
    view.current_element().common.style.position = Some(Point::new(x, y));
}

pub(crate) fn scale(view: &mut View, factor: f32) {
    if !(factor.is_finite() && factor > 0.0) {
        violation(format!("the scale {factor} is not above 0"));
    }
    view.current_element().common.scale = Some(factor);
}

pub(crate) fn opacity(view: &mut View, value: f32) {
    if !(0.0..=1.0).contains(&value) {
        violation(format!("the opacity {value} is not between 0 and 1"));
    }
    view.current_element().common.opacity = Some(value);
}

pub(crate) fn background(view: &mut View, value: ffi::Color) {
    view.current_element().common.background = Some(color(value));
}

pub(crate) fn tooltip(view: &mut View, text: &str) {
    view.current_element().common.tooltip = Some(text.to_owned());
}

pub(crate) fn key(view: &mut View, key: &str) {
    view.current_element().common.key = Some(key_of(key));
}

pub(crate) fn transition(view: &mut View, style: ffi::TransitionStyle) {
    let chosen: Transition = match style {
        ffi::TransitionStyle::Fade => fade(),
        ffi::TransitionStyle::FadeScale => fade_scale(),
        ffi::TransitionStyle::Pop => pop(),
        ffi::TransitionStyle::SlideFromTop => slide(Edge::Top),
        ffi::TransitionStyle::SlideFromBottom => slide(Edge::Bottom),
        ffi::TransitionStyle::SlideFromLeading => slide(Edge::Leading),
        ffi::TransitionStyle::SlideFromTrailing => slide(Edge::Trailing),
    };
    view.current_element().common.transition = Some(chosen);
}

pub(crate) fn anchor(view: &mut View, target: &str, edge: ffi::AnchorEdge, x: f32, y: f32) {
    let edge = match edge {
        ffi::AnchorEdge::BelowStart => AnchorEdge::BelowStart,
        ffi::AnchorEdge::BelowEnd => AnchorEdge::BelowEnd,
        ffi::AnchorEdge::AboveStart => AnchorEdge::AboveStart,
        ffi::AnchorEdge::AboveEnd => AnchorEdge::AboveEnd,
        ffi::AnchorEdge::BesideTop => AnchorEdge::BesideTop,
        ffi::AnchorEdge::BesideLeftTop => AnchorEdge::BesideLeftTop,
        ffi::AnchorEdge::Center => AnchorEdge::Center,
        ffi::AnchorEdge::Cover => AnchorEdge::Cover,
    };
    let x = finite(f64::from(x), "anchor offset") as f32;
    let y = finite(f64::from(y), "anchor offset") as f32;
    let node = view.current();
    if !matches!(node.kind, Kind::Popover { .. } | Kind::PopupMenu { .. }) {
        node.refuse("anchor");
    }
    node.common.anchor = Some(Anchor::new(key_of(target), edge).offset(x, y));
}

pub(crate) fn gap(view: &mut View, value: f32) {
    let value = extent(value, "gap");
    let node = view.current();
    match &mut node.kind {
        Kind::Flex(flex) => flex.edit(move |flex| flex.gap(value)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.gap(Space::from(value))),
        Kind::List(list) => list.edit(move |list| list.gap(Space::from(value))),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.gap(Space::from(value)));
        }
        Kind::Box { stack, .. }
        | Kind::Section { stack, .. }
        | Kind::Card { stack }
        | Kind::ButtonFrame { stack, .. }
        | Kind::Sheet { stack, .. }
        | Kind::Popover { stack, .. } => stack.gap = value,
        _ => node.refuse("gap"),
    }
}

fn set_padding(view: &mut View, inset: Inset) {
    let node = view.current();
    match &mut node.kind {
        Kind::Flex(flex) => flex.edit(move |flex| flex.padding(inset)),
        Kind::Box { container, .. } => container.edit(move |container| container.padding(inset)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.padding(inset)),
        Kind::List(list) => list.edit(move |list| list.padding(inset)),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.padding(inset))
        }
        Kind::Section { stack, .. }
        | Kind::Card { stack }
        | Kind::Sheet { stack, .. }
        | Kind::Popover { stack, .. } => stack.padding = inset,
        _ => node.refuse("padding"),
    }
}

pub(crate) fn padding(view: &mut View, value: f32) {
    set_padding(view, Inset::all(extent(value, "padding")));
}

pub(crate) fn padding_edges(view: &mut View, top: f32, right: f32, bottom: f32, left: f32) {
    set_padding(view, edges(top, right, bottom, left, "padding"));
}

pub(crate) fn align_items(view: &mut View, value: ffi::Align) {
    let value = align(value);
    let node = view.current();
    match &mut node.kind {
        Kind::Flex(flex) => flex.edit(move |flex| flex.align_items(value)),
        _ => node.refuse("item alignment"),
    }
}

pub(crate) fn justify(view: &mut View, value: ffi::Justification) {
    let value = crate::values::justify(value);
    let node = view.current();
    match &mut node.kind {
        Kind::Flex(flex) => flex.edit(move |flex| flex.justify(value)),
        _ => node.refuse("justification"),
    }
}

pub(crate) fn wrap_items(view: &mut View) {
    let node = view.current();
    match &mut node.kind {
        Kind::Flex(flex) => flex.edit(|flex| flex.wrap_items()),
        _ => node.refuse("wrapping"),
    }
}

pub(crate) fn surface(view: &mut View) {
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => container.edit(|container| container.surface()),
        _ => node.refuse("surface"),
    }
}

pub(crate) fn glass(view: &mut View) {
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => container.edit(|container| container.surface().glass()),
        _ => node.refuse("glass"),
    }
}

pub(crate) fn corner_radius(view: &mut View, radius: f32) {
    let radius = extent(radius, "corner radius");
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => container.edit(move |container| container.radius(radius)),
        Kind::Picture(picture) => picture.edit(move |picture| picture.radius(radius)),
        _ => node.refuse("corner radius"),
    }
}

pub(crate) fn border(view: &mut View, width: f32, value: ffi::Color) {
    let width = extent(width, "border width");
    let value = color(value);
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => {
            container.edit(move |container| container.border(width, value));
        }
        _ => node.refuse("border"),
    }
}

pub(crate) fn clip_content(view: &mut View) {
    let node = view.current();
    match &mut node.kind {
        Kind::Box { container, .. } => container.edit(|container| container.clip_content()),
        _ => node.refuse("clipping"),
    }
}

fn styled_text(
    view: &mut View,
    what: &str,
    edit: impl FnOnce(rikka_ui::Text) -> rikka_ui::Text + 'static,
) {
    let node = view.current();
    match &mut node.kind {
        Kind::Text(text) => text.edit(edit),
        _ => node.refuse(what),
    }
}

pub(crate) fn text_style(view: &mut View, value: ffi::TextRole) {
    let value = role(value);
    styled_text(view, "text style", move |text| text.role(value));
}

pub(crate) fn tone(view: &mut View, value: ffi::TextTone) {
    let value = crate::values::tone(value);
    styled_text(view, "text tone", move |text| text.color(value));
}

pub(crate) fn text_size(view: &mut View, size: f32) {
    let size = px(size, "text size");
    styled_text(view, "text size", move |text| text.text_size(size));
}

pub(crate) fn text_weight(view: &mut View, value: ffi::FontWeight) {
    let value = weight(value);
    styled_text(view, "text weight", move |text| text.weight(value));
}

pub(crate) fn text_italic(view: &mut View) {
    styled_text(view, "italic style", |text| text.italic());
}

pub(crate) fn text_align(view: &mut View, value: ffi::TextAlignment) {
    let value = match value {
        ffi::TextAlignment::Left => TextAlign::Left,
        ffi::TextAlignment::Center => TextAlign::Center,
        ffi::TextAlignment::Right => TextAlign::Right,
        ffi::TextAlignment::Justified => TextAlign::Justified,
    };
    styled_text(view, "text alignment", move |text| text.align(value));
}

pub(crate) fn text_no_wrap(view: &mut View) {
    styled_text(view, "wrapping", |text| text.no_wrap());
}

pub(crate) fn text_monospace(view: &mut View) {
    styled_text(view, "font family", |text| text.family(Family::Monospace));
}

pub(crate) fn text_line_height(view: &mut View, line_height: f32) {
    let line_height = extent(line_height, "line height");
    styled_text(view, "line height", move |text| {
        text.line_height(line_height)
    });
}

pub(crate) fn tint(view: &mut View, value: ffi::Color) {
    let value = color(value);
    let node = view.current();
    match &mut node.kind {
        Kind::Text(text) => text.edit(move |text| text.color(value)),
        Kind::Icon(icon) => icon.edit(move |icon| icon.color(value)),
        Kind::Slider(slider) => slider.edit(move |slider| slider.color(value)),
        Kind::Progress(progress) => progress.edit(move |progress| progress.color(value)),
        Kind::Spinner(spinner) => spinner.edit(move |spinner| spinner.color(value)),
        Kind::Gauge(gauge) => gauge.edit(move |gauge| gauge.color(value)),
        Kind::ListItem(item) => item.edit(move |item| item.tint(value)),
        Kind::NavItem(item) => item.edit(move |item| item.tint(value)),
        Kind::Tab(tab) => tab.edit(move |tab| tab.tint(value)),
        Kind::Banner(banner) => banner.edit(move |banner| banner.tint(value)),
        Kind::StatusPage(status) => status.edit(move |status| status.tint(value)),
        _ => node.refuse("tint"),
    }
}

pub(crate) fn icon_size(view: &mut View, size: f32) {
    let size = px(size, "icon size");
    let node = view.current();
    match &mut node.kind {
        Kind::Icon(icon) => icon.edit(move |icon| icon.extent(size)),
        Kind::Spinner(spinner) => spinner.edit(move |spinner| spinner.extent(size)),
        Kind::Gauge(gauge) => gauge.edit(move |gauge| gauge.extent(size)),
        _ => node.refuse("icon size"),
    }
}

pub(crate) fn icon_filled(view: &mut View) {
    let node = view.current();
    match &mut node.kind {
        Kind::Icon(icon) => icon.edit(|icon| icon.filled()),
        _ => node.refuse("fill"),
    }
}

pub(crate) fn enabled(view: &mut View, enabled: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::Button(button) | Kind::ButtonFrame { button, .. } => {
            button.edit(move |button| button.enabled(enabled));
        }
        Kind::Checkbox(checkbox) => checkbox.edit(move |checkbox| checkbox.enabled(enabled)),
        Kind::Toggle(toggle) => toggle.edit(move |toggle| toggle.enabled(enabled)),
        Kind::Radio(radio) => radio.edit(move |radio| radio.enabled(enabled)),
        Kind::Slider(slider) => slider.edit(move |slider| slider.enabled(enabled)),
        Kind::NumberField(number) => number.edit(move |number| number.enabled(enabled)),
        Kind::Segmented(segmented) => segmented.edit(move |segmented| segmented.enabled(enabled)),
        Kind::Combo(combo) => combo.edit(move |combo| combo.enabled(enabled)),
        Kind::TextField(field) => field.edit(move |field| field.enabled(enabled)),
        Kind::Tab(tab) => tab.edit(move |tab| tab.enabled(enabled)),
        Kind::ListItem(item) => item.edit(move |item| item.enabled(enabled)),
        Kind::MenuEntry(entry) | Kind::Submenu(entry) => entry.enabled = enabled,
        _ => node.refuse("enabled state"),
    }
}

pub(crate) fn control_size(view: &mut View, size: ffi::SizeClass) {
    let size = size_class(size);
    let node = view.current();
    match &mut node.kind {
        Kind::Button(button) | Kind::ButtonFrame { button, .. } => {
            button.edit(move |button| button.control_size(size));
        }
        Kind::Checkbox(checkbox) => checkbox.edit(move |checkbox| checkbox.control_size(size)),
        Kind::Toggle(toggle) => toggle.edit(move |toggle| toggle.control_size(size)),
        Kind::Radio(radio) => radio.edit(move |radio| radio.control_size(size)),
        Kind::Slider(slider) => slider.edit(move |slider| slider.control_size(size)),
        Kind::NumberField(number) => number.edit(move |number| number.control_size(size)),
        Kind::Segmented(segmented) => segmented.edit(move |segmented| segmented.control_size(size)),
        Kind::Combo(combo) => combo.edit(move |combo| combo.control_size(size)),
        Kind::TextField(field) => field.edit(move |field| field.control_size(size)),
        Kind::Progress(progress) => progress.edit(move |progress| progress.control_size(size)),
        Kind::Tabs(tabs) => tabs.edit(move |tabs| tabs.control_size(size)),
        Kind::NavList(nav) => nav.edit(move |nav| nav.control_size(size)),
        Kind::List(list) => list.edit(move |list| list.control_size(size)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.control_size(size)),
        _ => node.refuse("control size"),
    }
}

pub(crate) fn button_style(view: &mut View, variant: ffi::ButtonVariant) {
    let variant = match variant {
        ffi::ButtonVariant::Primary => rikka_widgets::ButtonVariant::Primary,
        ffi::ButtonVariant::Secondary => rikka_widgets::ButtonVariant::Secondary,
        ffi::ButtonVariant::Ghost => rikka_widgets::ButtonVariant::Ghost,
        ffi::ButtonVariant::Danger => rikka_widgets::ButtonVariant::Danger,
    };
    let node = view.current();
    match &mut node.kind {
        Kind::Button(button) | Kind::ButtonFrame { button, .. } => {
            button.edit(move |button| button.variant(variant));
        }
        _ => node.refuse("button style"),
    }
}

pub(crate) fn loading(view: &mut View, loading: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::Button(button) | Kind::ButtonFrame { button, .. } => {
            button.edit(move |button| button.loading(loading));
        }
        _ => node.refuse("loading state"),
    }
}

pub(crate) fn range(view: &mut View, min: f64, max: f64) {
    let min = finite(min, "minimum") as f32;
    let max = finite(max, "maximum") as f32;
    if max < min {
        violation(format!("the range {min} to {max} is reversed"));
    }
    let node = view.current();
    match &mut node.kind {
        Kind::Slider(slider) => slider.edit(move |slider| slider.range(min..=max)),
        Kind::NumberField(number) => number.edit(move |number| number.range(min..=max)),
        _ => node.refuse("range"),
    }
}

pub(crate) fn step(view: &mut View, step: f64) {
    let step = finite(step, "step") as f32;
    if step <= 0.0 {
        violation(format!("the step {step} is not positive"));
    }
    let node = view.current();
    match &mut node.kind {
        Kind::Slider(slider) => slider.edit(move |slider| slider.step(step)),
        Kind::NumberField(number) => number.edit(move |number| number.step(step)),
        _ => node.refuse("step"),
    }
}

pub(crate) fn ticks(view: &mut View, count: u32) {
    let node = view.current();
    match &mut node.kind {
        Kind::Slider(slider) => slider.edit(move |slider| slider.ticks(count as usize)),
        _ => node.refuse("ticks"),
    }
}

pub(crate) fn decimals(view: &mut View, decimals: u32) {
    let decimals = u8::try_from(decimals)
        .unwrap_or_else(|_| violation(format!("{decimals} decimals are more than 255")));
    let node = view.current();
    match &mut node.kind {
        Kind::NumberField(number) => number.edit(move |number| number.decimals(decimals)),
        _ => node.refuse("decimals"),
    }
}

pub(crate) fn unit(view: &mut View, unit: &str) {
    let unit = unit.to_owned();
    let node = view.current();
    match &mut node.kind {
        Kind::NumberField(number) => number.edit(move |number| number.unit(unit)),
        _ => node.refuse("unit"),
    }
}

pub(crate) fn placeholder(view: &mut View, text: &str) {
    let text = text.to_owned();
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.placeholder(text)),
        Kind::TextEditor(editor) => editor.edit(move |editor| editor.placeholder(text)),
        Kind::Combo(combo) => combo.edit(move |combo| combo.placeholder(text)),
        _ => node.refuse("placeholder"),
    }
}

pub(crate) fn secret(view: &mut View, secret: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.secret(secret)),
        _ => node.refuse("secret mode"),
    }
}

pub(crate) fn max_length(view: &mut View, length: usize) {
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.max_len(length)),
        _ => node.refuse("length limit"),
    }
}

pub(crate) fn invalid(view: &mut View, invalid: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.error(invalid)),
        _ => node.refuse("invalid state"),
    }
}

pub(crate) fn autofocus(view: &mut View, autofocus: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::TextField(field) => field.edit(move |field| field.autofocus(autofocus)),
        Kind::TextEditor(editor) => editor.edit(move |editor| editor.autofocus(autofocus)),
        _ => node.refuse("autofocus"),
    }
}

fn editor(
    view: &mut View,
    what: &str,
    edit: impl FnOnce(rikka_widgets::TextEditor) -> rikka_widgets::TextEditor + 'static,
) {
    let node = view.current();
    match &mut node.kind {
        Kind::TextEditor(editor) => editor.edit(edit),
        _ => node.refuse(what),
    }
}

pub(crate) fn read_only(view: &mut View, read_only: bool) {
    editor(view, "read-only mode", move |editor| {
        editor.read_only(read_only)
    });
}

pub(crate) fn wrap_lines(view: &mut View, wrap: bool) {
    editor(view, "line wrapping", move |editor| editor.wrap(wrap));
}

pub(crate) fn code(view: &mut View, code: bool) {
    editor(view, "code assists", move |editor| editor.code(code));
}

pub(crate) fn line_numbers(view: &mut View, shown: bool) {
    editor(view, "line numbers", move |editor| editor.gutter(shown));
}

pub(crate) fn syntax(view: &mut View, language: &str) {
    let language = rikka_syntax::Language::named(language)
        .unwrap_or_else(|| violation(format!("`{language}` is not a known language")));
    editor(view, "syntax", move |editor| editor.language(language));
}

pub(crate) fn image_fit(view: &mut View, fit: ffi::Fit) {
    let fit = match fit {
        ffi::Fit::Stretch => ImageFit::Stretch,
        ffi::Fit::Contain => ImageFit::Contain,
        ffi::Fit::Cover => ImageFit::Cover,
        ffi::Fit::Natural => ImageFit::Natural,
        ffi::Fit::ScaleDown => ImageFit::ScaleDown,
    };
    let node = view.current();
    match &mut node.kind {
        Kind::Picture(picture) => picture.edit(move |picture| picture.fit(fit)),
        _ => node.refuse("fit"),
    }
}

pub(crate) fn secondary_text(view: &mut View, text: &str) {
    let text = text.to_owned();
    let node = view.current();
    match &mut node.kind {
        Kind::ListItem(item) => item.edit(move |item| item.secondary(text)),
        _ => node.refuse("secondary text"),
    }
}

pub(crate) fn trailing_text(view: &mut View, text: &str) {
    let text = text.to_owned();
    let node = view.current();
    match &mut node.kind {
        Kind::ListItem(item) => item.edit(move |item| item.trailing(text)),
        _ => node.refuse("trailing text"),
    }
}

pub(crate) fn leading_icon(view: &mut View, icon: &str) {
    let icon = symbol(icon);
    let node = view.current();
    match &mut node.kind {
        Kind::ListItem(item) => item.edit(move |item| item.icon(icon)),
        Kind::NavItem(item) => item.edit(move |item| item.icon(icon)),
        Kind::Tab(tab) => tab.edit(move |tab| tab.icon(icon)),
        Kind::Banner(banner) => banner.edit(move |banner| banner.symbol(icon)),
        Kind::MenuEntry(entry) | Kind::Submenu(entry) => entry.icon = Some(icon),
        _ => node.refuse("leading icon"),
    }
}

pub(crate) fn check(view: &mut View, state: ffi::CheckState) {
    let state = check_state(state);
    let node = view.current();
    match &mut node.kind {
        Kind::ListItem(item) => item.edit(move |item| item.check(state)),
        _ => node.refuse("check"),
    }
}

pub(crate) fn closable(view: &mut View, closable: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::Tab(tab) => tab.edit(move |tab| tab.closable(closable)),
        _ => node.refuse("close button"),
    }
}

pub(crate) fn shortcut_text(view: &mut View, text: &str) {
    let node = view.current();
    match &mut node.kind {
        Kind::MenuEntry(entry) => entry.shortcut = Some(text.to_owned()),
        _ => node.refuse("shortcut text"),
    }
}

pub(crate) fn checked(view: &mut View, checked: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::MenuEntry(entry) => entry.checked = checked,
        _ => node.refuse("checked state"),
    }
}

pub(crate) fn selection(view: &mut View, index: usize) {
    let index = selected_index(index);
    let node = view.current();
    match &mut node.kind {
        Kind::List(list) => list.edit(move |list| list.selected(index)),
        Kind::LazyList { list, selected } => {
            selected.set(index);
            list.edit(move |list| list.selected(index));
        }
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.selected(index));
        }
        _ => node.refuse("selection"),
    }
}

pub(crate) fn item_extent(view: &mut View, value: f32) {
    let value = px(value, "item extent");
    let node = view.current();
    match &mut node.kind {
        Kind::List(list) => list.edit(move |list| list.uniform(value)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.uniform(value)),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.uniform(value))
        }
        _ => node.refuse("item extent"),
    }
}

pub(crate) fn scroll_follow(view: &mut View, follow: bool) {
    let node = view.current();
    match &mut node.kind {
        Kind::Scroll { follow: following } => *following = follow,
        Kind::List(list) => list.edit(move |list| list.follow(follow)),
        Kind::LazyList { list, .. } => list.edit(move |list| list.follow(follow)),
        Kind::Collection(collection) => {
            collection.edit(move |collection| collection.follow(follow))
        }
        _ => node.refuse("follow mode"),
    }
}
