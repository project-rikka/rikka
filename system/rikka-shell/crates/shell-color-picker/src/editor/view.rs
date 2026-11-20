use std::rc::Rc;

use rikka_color_formats::{Grouping, Rgb, Template};
use rikka_settings::session::color_picker::Format;
use rikka_theme::Corner;
use rikka_types::{Color, Key, Modifiers, Point, PointerButton};
use rikka_ui::{
    Align, Anchor, AnchorEdge, Element, Event, EventCtx, Handler, Inset, Length, MaterialSymbol,
    Space, Styled, TextStyled, View, palette, text, tr,
};
use rikka_widgets::{
    ControlSize, Surface, button, color_picker, column, column_of, container, icon, lazy_column,
    menu, menu_header, menu_item, menu_separator, row, rule, rule_upright, scrollable, spacer,
    status_page,
};
use shell_kit::floating;

use crate::editor::state::{Editor, MenuAnchor};

const WIDTH: f32 = 580.0;
const DOWN: f32 = 10.0;
const BODY: f32 = 344.0;
const BORDER: f32 = 1.0;
const EDGE: f32 = 14.0;
pub(crate) const GAP: f32 = 10.0;
const ICON: f32 = 20.0;
const SMALL_ICON: f32 = 18.0;

const HISTORY: f32 = 72.0;
const CELL: f32 = 44.0;
const SWATCH: f32 = 30.0;
const RING: f32 = 2.0;
const RING_GAP: f32 = 3.0;

const BAR: f32 = 76.0;
const SHADE: f32 = 40.0;
const CURRENT: f32 = 120.0;

const MENU: f32 = 232.0;
const TUNE_GAP: f32 = 8.0;

const MARK: &str = "color-picker.history-mark";
const MENU_KEY: &str = "color-picker.history-menu";
const CURRENT_KEY: &str = "color-picker.current";
const TUNE_KEY: &str = "color-picker.tune";

pub(crate) fn view(editor: &Editor) -> impl View {
    floating(editor.spot(), window(editor), Editor::dismiss)
        .on_key(on_key)
        .width(WIDTH)
        .down(DOWN / 100.0)
}

fn on_key(editor: &mut Editor, key: Key, modifiers: Modifiers) -> bool {
    if editor.tuning().is_some() || editor.menu().is_some() {
        return false;
    }
    match key {
        Key::Escape => editor.dismiss(),
        Key::Delete => editor.remove(),
        Key::Char('c' | 'C') if modifiers.control => editor.copy_current(),
        _ => return false,
    }
    true
}

fn window(editor: &Editor) -> impl View {
    let palette = palette();
    let window = column! {
        heading();
        rule();
        body(editor)
    }
    .fill_width();
    container(window)
        .background(palette.background)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
}

fn heading() -> impl View {
    let palette = palette();
    row! {
        icon(MaterialSymbol::COLORIZE)
            .extent(Length::px(ICON))
            .color(palette.accent);
        text(tr!("shell.color-picker.name")).label();
        spacer().grow(1.0);
        button(
            row! {
                icon(MaterialSymbol::COLORIZE).extent(Length::px(SMALL_ICON));
                text(tr!("shell.color-picker.pick"))
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .control_size(ControlSize::Small)
        .on_press(Editor::pick);
        button(icon(MaterialSymbol::SETTINGS).extent(Length::px(ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .on_press(Editor::settings);
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .on_press(Editor::dismiss)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(EDGE, GAP))
    .fill_width()
}

fn body(editor: &Editor) -> Element {
    let Some(current) = editor.current() else {
        return status_page(MaterialSymbol::COLORIZE, tr!("shell.color-picker.empty"))
            .action(
                button(text(tr!("shell.color-picker.pick")))
                    .primary()
                    .on_press(Editor::pick),
            )
            .build()
            .height(BODY)
            .fill_width();
    };
    row! {
        history(editor);
        rule_upright();
        bar(editor, current);
        formats(editor, current)
    }
    .height(BODY)
    .fill_width()
    .build()
}

fn history(editor: &Editor) -> Element {
    let colors: Rc<[Rgb]> = Rc::from(editor.colors());
    let chosen: Rc<[bool]> = (0..colors.len())
        .map(|index| editor.is_chosen(index))
        .collect();
    let open_menu = Handler::new(|editor: &mut Editor, index: usize, at: Point| {
        editor.open_menu(index, at);
    });
    let drawn = Rc::clone(&colors);
    let list = lazy_column(colors.len(), move |slot| {
        swatch(drawn[slot.index], chosen[slot.index])
    })
    .uniform(CELL)
    .selected(editor.focus())
    .focus(editor.focus())
    .on_focus(Editor::focused)
    .on_pointer(move |event, ctx, surface| open_menu_on_click(&open_menu, event, ctx, surface))
    .padding(Inset::xy(0.0, GAP / 2.0))
    .grow(1.0)
    .fill_width();

    column! {
        spacer().width(0.0).height(0.0).build().keyed(MARK);
        list;
        editor.menu().map(history_menu)
    }
    .width(HISTORY)
    .fill_height()
    .build()
}

fn open_menu_on_click(
    open_menu: &Handler<(usize, Point)>,
    event: &Event,
    ctx: &mut EventCtx<'_>,
    surface: &Surface<'_>,
) -> bool {
    let Event::PointerPressed {
        button: PointerButton::Secondary,
        position,
        ..
    } = *event
    else {
        return false;
    };
    let Some(index) = surface.at(position) else {
        return false;
    };
    let origin = ctx.bounds().origin;
    ctx.set_handled();
    ctx.call(
        open_menu,
        (
            index,
            Point::new(position.x - origin.x, position.y - origin.y),
        ),
    );
    true
}

fn history_menu(anchor: MenuAnchor) -> Element {
    menu()
        .items([
            menu_item(tr!("shell.color-picker.remove"))
                .icon(MaterialSymbol::DELETE)
                .on_press(Editor::remove),
            menu_separator(),
            menu_header(tr!("shell.color-picker.export.name")),
            menu_item(tr!("shell.color-picker.export.by-color"))
                .icon(MaterialSymbol::PALETTE)
                .on_press(|editor: &mut Editor| editor.export(Grouping::ByColor)),
            menu_item(tr!("shell.color-picker.export.by-format"))
                .icon(MaterialSymbol::DATA_OBJECT)
                .on_press(|editor: &mut Editor| editor.export(Grouping::ByFormat)),
        ])
        .width(MENU)
        .on_dismiss(Editor::dismiss_menu)
        .build()
        .keyed((MENU_KEY, anchor.index))
        .anchored(Anchor::new(MARK, AnchorEdge::BesideTop).offset(anchor.at.x, anchor.at.y))
}

fn swatch(rgb: Rgb, chosen: bool) -> impl View {
    let palette = palette();
    let ring = match chosen {
        true => palette.accent,
        false => Color::TRANSPARENT,
    };
    let fill = container(spacer().width(SWATCH).height(SWATCH))
        .background(rgb.color())
        .border(BORDER, palette.outline)
        .radius(Corner::Control);
    let ringed = container(fill)
        .padding(Inset::all(RING_GAP))
        .border(RING, ring)
        .radius(Corner::Panel);
    container(ringed).center().fill_width().height(CELL)
}

fn bar(editor: &Editor, current: Rgb) -> Element {
    let shades = current.shades();
    let shade = |rgb: Rgb| {
        container(spacer().fill_width().height(SHADE))
            .background(rgb.color())
            .on_press(move |editor: &mut Editor| editor.adopt(rgb))
            .fill_width()
    };
    let strip = column! {
        shade(shades.lightest);
        shade(shades.lighter);
        container(spacer().fill_width().height(CURRENT))
            .background(current.color())
            .on_press(Editor::tune)
            .fill_width()
            .build()
            .keyed(CURRENT_KEY);
        shade(shades.darker);
        shade(shades.darkest)
    }
    .fill_width();
    column! {
        container(strip)
            .border(BORDER, palette().outline)
            .radius(Corner::Control)
            .clip_content()
            .width(BAR);
        editor.tuning().map(tune)
    }
    .padding(Inset::xy(GAP, GAP))
    .build()
}

fn tune(tuning: Rgb) -> Element {
    let panel = column! {
        color_picker(tuning.color())
            .alpha(false)
            .on_change(Editor::tuned);
        row! {
            spacer().grow(1.0);
            button(text(tr!("shell.color-picker.cancel")))
                .secondary()
                .control_size(ControlSize::Small)
                .on_press(Editor::cancel_tune);
            button(text(tr!("shell.color-picker.select")))
                .primary()
                .control_size(ControlSize::Small)
                .on_press(Editor::apply_tune)
        }
        .gap(Space::TIGHT)
        .fill_width()
    }
    .gap(Space::NORMAL);
    rikka_widgets::popover(panel)
        .on_dismiss(Editor::cancel_tune)
        .build()
        .keyed(TUNE_KEY)
        .anchored(Anchor::beside(CURRENT_KEY).offset(TUNE_GAP, 0.0))
}

fn formats(editor: &Editor, current: Rgb) -> Element {
    let copied = editor.copied();
    let rows = editor
        .formats()
        .into_iter()
        .enumerate()
        .map(|(row, format)| format_row(row, &format, current, copied == Some(row)));
    scrollable(
        column_of(rows)
            .gap(Space::TIGHT)
            .padding(Inset::xy(0.0, GAP))
            .fill_width(),
    )
    .grow(1.0)
    .fill_height()
    .build()
}

fn format_row(row: usize, format: &Format, current: Rgb, copied: bool) -> Element {
    let rendered = Template::parse(&format.format).render(current);
    let copying = rendered.clone();
    let symbol = match copied {
        true => MaterialSymbol::CHECK,
        false => MaterialSymbol::CONTENT_COPY,
    };
    row! {
        column! {
            text(format.name.to_uppercase()).caption().muted().no_wrap();
            text(rendered).no_wrap()
        }
        .gap(Space::from(2.0))
        .grow(1.0)
        .shrink(1.0)
        .min_width(Length::px(0.0));
        button(icon(symbol).extent(Length::px(SMALL_ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .on_press(move |editor: &mut Editor| editor.copy(row, copying.clone()))
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(GAP, 4.0))
    .fill_width()
    .build()
}
