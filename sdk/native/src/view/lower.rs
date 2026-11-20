use rikka_ui::{
    Align, Anchor, Element, Handler, IconGlyph, Label, Length, Space, Style, Styled, View, fade,
};
use rikka_widgets::{
    Sheet, card, column_of, container, context_area, field, list, menu, page, page_stack, popover,
    row_of, rule, rule_upright, scrim, scrollable, section, sheet, spacer, split,
};

use crate::abi::violation;
use crate::view::STAGE;
use crate::view::menu::{self, MenuEntry};
use crate::view::node::{Common, Kind, Node, Stack};

pub(crate) fn elements(nodes: Vec<Node>) -> Vec<Element> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        lower(node, &mut out);
    }
    out
}

pub(crate) fn entries(nodes: Vec<Node>) -> Vec<MenuEntry> {
    nodes
        .into_iter()
        .map(|node| match node.kind {
            Kind::MenuEntry(entry) => entry,
            Kind::Submenu(mut entry) => {
                entry.children = entries(node.children);
                entry
            }
            _ => unreachable!("menus admit only menu entries"),
        })
        .collect()
}

fn lower(node: Node, out: &mut Vec<Element>) {
    let Node {
        kind,
        common,
        children,
    } = node;
    match kind {
        Kind::Sheet { stack, dismiss } => {
            let mut panel = sheet(stacked(children, stack));
            if let Some(dismiss) = &dismiss {
                panel = panel.on_dismiss(dismiss.clone());
            }
            sheeted(panel, dismiss, common, out);
        }
        Kind::Unsaved(prompt) => sheeted(*prompt, None, common, out),
        kind => {
            let anchored = matches!(kind, Kind::Popover { .. } | Kind::PopupMenu { .. });
            if anchored && common.anchor.is_none() {
                violation(format!("{} has no rk_anchor", kind.name()));
            }
            out.push(decorate(element(kind, children), common));
        }
    }
}

fn sheeted(panel: Sheet, dismiss: Option<Handler>, mut common: Common, out: &mut Vec<Element>) {
    let mut veil = scrim();
    if let Some(dismiss) = dismiss {
        veil = veil.on_dismiss(dismiss);
    }
    let mut veil = veil.transition(fade()).anchored(Anchor::covering(STAGE));
    if let Some(key) = common.key {
        veil = veil.keyed((key, STAGE));
    }
    out.push(veil);
    common.anchor = Some(Anchor::centered_on(STAGE));
    out.push(decorate(panel.build(), common));
}

fn single(node: Node) -> Element {
    let mut out = Vec::new();
    lower(node, &mut out);
    match out.len() {
        1 => out.pop().expect("one element"),
        _ => column_of(out).build(),
    }
}

fn filling(mut element: Element) -> Element {
    let style = element.style_mut();
    style.width = style.width.or(Some(Length::FILL));
    style.height = style.height.or(Some(Length::FILL));
    element
}

fn only(mut children: Vec<Node>) -> Element {
    single(
        children
            .pop()
            .expect("validated at rk_end: exactly one child"),
    )
}

fn stacked(children: Vec<Node>, stack: Stack) -> Element {
    column_of(elements(children))
        .gap(stack.gap)
        .padding(stack.padding)
        .fill_width()
        .build()
}

fn built<T: 'static>(builder: crate::view::node::Built<T>) -> T {
    builder.finish(|| unreachable!("this element is built when declared"))
}

fn element(kind: Kind, children: Vec<Node>) -> Element {
    match kind {
        Kind::Flex(flex) => built(flex).extend(elements(children)).build(),
        Kind::Box {
            container: frame,
            stack,
        } => frame.finish(|| container(stacked(children, stack))).build(),
        Kind::Scroll { follow } => scrollable(stacked(children, Stack::new(Space::NONE, 0.0)))
            .follow(follow)
            .build(),
        Kind::Page => page(elements(children)).build(),
        Kind::Section { heading, stack } => section(heading, stacked(children, stack)).build(),
        Kind::Card { stack } => card(stacked(children, stack)).build(),
        Kind::Field { label } => field(label, only(children)).build(),
        Kind::Button(button) => built(button).build(),
        Kind::ButtonFrame { button, stack } => button
            .finish(|| {
                rikka_widgets::button(
                    row_of(elements(children))
                        .gap(stack.gap)
                        .align_items(Align::Center),
                )
            })
            .build(),
        Kind::Split {
            axis,
            ratio,
            resize,
        } => {
            let mut panes = children.into_iter().map(|pane| filling(single(pane)));
            let first = panes.next().expect("validated at rk_end: two children");
            let second = panes.next().expect("validated at rk_end: two children");
            let divided = split(axis, ratio, first, second);
            match resize {
                Some(resize) => divided.on_resize(resize).build(),
                None => divided.build(),
            }
        }
        Kind::PageStack { page } => page_stack(page, only(children)).build(),
        Kind::Popover { stack, dismiss } => {
            let shown = popover(stacked(children, stack));
            match dismiss {
                Some(dismiss) => shown.on_dismiss(dismiss).build(),
                None => shown.build(),
            }
        }
        Kind::StatusPage(status) => {
            let status = built(status);
            match children.is_empty() {
                true => status.build(),
                false => status
                    .action(row_of(elements(children)).gap(Space::SNUG))
                    .build(),
            }
        }
        Kind::PopupMenu { dismiss } => {
            let shown = menu().items(menu::items(&entries(children)));
            match dismiss {
                Some(dismiss) => shown.on_dismiss(dismiss).build(),
                None => shown.build(),
            }
        }
        Kind::MenuBar(bar) => children
            .into_iter()
            .fold(built(bar), |bar, child| match child.kind {
                Kind::Menu { title } => bar.menu(title, menu::items(&entries(child.children))),
                _ => unreachable!("menu bars admit only menus"),
            })
            .build(),
        Kind::Segmented(segmented) => {
            let segments: Vec<_> = children
                .into_iter()
                .map(|child| match child.kind {
                    Kind::Segment(segment) => segment,
                    _ => unreachable!("segmented controls admit only segments"),
                })
                .collect();
            let icons: Vec<IconGlyph> = segments
                .iter()
                .filter_map(|segment| segment.icon.map(IconGlyph::from))
                .collect();
            if !icons.is_empty() && icons.len() != segments.len() {
                violation("a segmented control mixes labelled and icon segments");
            }
            built(segmented)
                .segments(segments.into_iter().map(|segment| segment.label))
                .glyphs(icons)
                .build()
        }
        Kind::Combo(combo) => built(combo)
            .options(children.into_iter().map(|child| match child.kind {
                Kind::Option(label) => label,
                _ => unreachable!("combos admit only options"),
            }))
            .build(),
        Kind::Tabs(tabs) => built(tabs)
            .items(children.into_iter().map(|child| match child.kind {
                Kind::Tab(tab) => built(tab),
                _ => unreachable!("tabs admit only tabs"),
            }))
            .build(),
        Kind::NavList(nav) => built(nav)
            .items(children.into_iter().map(|child| match child.kind {
                Kind::NavItem(item) => built(item),
                _ => unreachable!("nav lists admit only nav items"),
            }))
            .build(),
        Kind::List(rows) => rows
            .finish(|| {
                list(children.into_iter().map(|child| match child.kind {
                    Kind::ListItem(item) => built(item),
                    _ => unreachable!("lists admit only list items"),
                }))
            })
            .build(),
        Kind::Text(text) => built(text).build(),
        Kind::Icon(icon) => built(icon).build(),
        Kind::Picture(picture) => built(picture).build(),
        Kind::Spacer => spacer().build(),
        Kind::Divider { vertical: false } => rule().build(),
        Kind::Divider { vertical: true } => rule_upright().build(),
        Kind::Checkbox(checkbox) => built(checkbox).build(),
        Kind::Toggle(toggle) => built(toggle).build(),
        Kind::Radio(radio) => built(radio).build(),
        Kind::Slider(slider) => built(slider).build(),
        Kind::NumberField(number) => built(number).build(),
        Kind::Progress(progress) => built(progress).build(),
        Kind::Spinner(spinner) => built(spinner).build(),
        Kind::Gauge(gauge) => built(gauge).build(),
        Kind::TextField(field) => built(field).build(),
        Kind::TextEditor(editor) => built(editor).build(),
        Kind::Banner(banner) => built(*banner).build(),
        Kind::LazyList { list: rows, .. } => built(rows).build(),
        Kind::Collection(collection) => built(collection).build(),
        Kind::Memo(memo) => built(memo).build(),
        Kind::Root(_)
        | Kind::Sheet { .. }
        | Kind::Unsaved(_)
        | Kind::ContextMenu { .. }
        | Kind::Menu { .. }
        | Kind::Submenu(_)
        | Kind::Segment(_)
        | Kind::Option(_)
        | Kind::Tab(_)
        | Kind::NavItem(_)
        | Kind::ListItem(_)
        | Kind::MenuEntry(_) => unreachable!("{} is never an element of its own", kind.name()),
    }
}

fn merge(into: &mut Style, from: &Style) {
    let Style {
        width,
        height,
        min_width,
        min_height,
        max_width,
        max_height,
        grow,
        shrink,
        default_grow,
        align_self,
        margin,
        position,
    } = *from;
    into.width = width.or(into.width);
    into.height = height.or(into.height);
    into.min_width = min_width.or(into.min_width);
    into.min_height = min_height.or(into.min_height);
    into.max_width = max_width.or(into.max_width);
    into.max_height = max_height.or(into.max_height);
    into.grow = grow.or(into.grow);
    into.shrink = shrink.or(into.shrink);
    into.default_grow = default_grow.or(into.default_grow);
    into.align_self = align_self.or(into.align_self);
    into.margin = margin.or(into.margin);
    into.position = position.or(into.position);
}

fn decorate(mut element: Element, common: Common) -> Element {
    let Common {
        style,
        key,
        tooltip,
        opacity,
        background,
        transition,
        anchor,
        shortcuts,
        commands,
        menu: entries,
        drops,
    } = common;
    let inner = Style {
        margin: None,
        position: None,
        ..style
    };
    let wrapping = |element: &mut Element| {
        if inner != Style::default() {
            merge(element.style_mut(), &inner);
        }
    };
    if !shortcuts.is_empty() || !commands.is_empty() {
        wrapping(&mut element);
        let mut frame = container(element);
        for (key, modifiers, handler) in shortcuts {
            frame = frame.on_chord(key, modifiers, handler);
        }
        for (command, handler) in commands {
            frame = frame.on_command(command, handler);
        }
        element = frame.build();
    }
    if let Some(entries) = entries {
        wrapping(&mut element);
        element = context_area(element, move || menu::items(&entries)).build();
    }
    if !drops.is_empty() {
        wrapping(&mut element);
        element = drops.wrap(element);
    }
    if style != Style::default() {
        merge(element.style_mut(), &style);
    }
    if let Some(key) = key {
        element = element.keyed(key);
    }
    if let Some(anchor) = anchor {
        element = element.anchored(anchor);
    }
    if let Some(tooltip) = tooltip {
        element = element.tooltip(Label::from(tooltip));
    }
    if let Some(opacity) = opacity {
        element = element.opacity(opacity);
    }
    if let Some(background) = background {
        element = element.background(background);
    }
    if let Some(transition) = transition {
        element = element.transition(transition);
    }
    element
}
