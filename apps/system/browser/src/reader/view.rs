use browser_preferences::{ReaderStyle, ReaderTheme};
use browser_reader::{FontSize, Spacing, Typeface, Width};
use rikka_app::prelude::*;
use rikka_input::Chord;
use rikka_types::Choice;
use rikka_web::web_view;
use rikka_widgets::column;

use crate::keys;
use crate::shell::Shell;
use crate::widgets::key_catcher;
use browser_model::TabId;

const BAR: f32 = 40.0;
const GLYPH: f32 = 18.0;

pub(crate) fn view(shell: &Shell, tab: TabId) -> Option<Element> {
    let reading = shell
        .reader
        .reading()
        .filter(|reading| reading.tab == tab)?;
    let style = shell.browser().preferences().reader_style();
    let page = web_view(&reading.view)
        .reserve_shortcuts(keys::reserved(false).chain([Chord::new(Modifiers::NONE, Key::Escape)]))
        .fill()
        .build()
        .keyed(("reader", tab));
    let body = column! {
        bar(style);
        container(spacer())
            .background(palette().outline.with_alpha(0.4))
            .height(1.0)
            .fill_width();
        page;
    }
    .fill();
    Some(key_catcher(
        body,
        |shell: &mut Shell, key: Key, modifiers: Modifiers| match (
            key,
            modifiers == Modifiers::NONE,
        ) {
            (Key::Escape, true) => {
                shell.close_reader();
                true
            }
            _ => false,
        },
    ))
}

fn position<T: PartialEq>(options: &[T], current: T) -> Option<usize> {
    options.iter().position(|each| *each == current)
}

fn bar(style: ReaderStyle) -> Element {
    let typeface = position(Typeface::ALL, style.typeface);
    let width = position(Width::ALL, style.width);
    let spacing = position(Spacing::ALL, style.spacing);
    let theme = position(ReaderTheme::ALL, style.theme);
    let smaller = ReaderStyle {
        size: style.size.smaller(),
        ..style
    };
    let larger = ReaderStyle {
        size: style.size.larger(),
        ..style
    };
    let typefaces = [tr!("browser.reader.sans"), tr!("browser.reader.serif")];
    let widths = [
        tr!("browser.reader.narrow"),
        tr!("browser.reader.medium"),
        tr!("browser.reader.wide"),
    ];
    let spacings = [
        tr!("browser.reader.compact"),
        tr!("browser.reader.normal"),
        tr!("browser.reader.relaxed"),
    ];
    let themes = [
        tr!("browser.reader.auto"),
        tr!("browser.reader.light"),
        tr!("browser.reader.sepia"),
        tr!("browser.reader.dark"),
    ];
    let pick_typeface = move |shell: &mut Shell, at: usize| {
        if let Some(typeface) = Typeface::ALL.get(at) {
            shell.restyle_reader(ReaderStyle {
                typeface: *typeface,
                ..style
            });
        }
    };
    let pick_width = move |shell: &mut Shell, at: usize| {
        if let Some(width) = Width::ALL.get(at) {
            shell.restyle_reader(ReaderStyle {
                width: *width,
                ..style
            });
        }
    };
    let pick_spacing = move |shell: &mut Shell, at: usize| {
        if let Some(spacing) = Spacing::ALL.get(at) {
            shell.restyle_reader(ReaderStyle {
                spacing: *spacing,
                ..style
            });
        }
    };
    let pick_theme = move |shell: &mut Shell, at: usize| {
        if let Some(theme) = ReaderTheme::ALL.get(at) {
            shell.restyle_reader(ReaderStyle {
                theme: *theme,
                ..style
            });
        }
    };
    toolbar()
        .gap(Space::NORMAL)
        .padding(Inset::xy(Space::NORMAL, Space::NONE))
        .group(
            tool_group([tool_item(
                icon(MaterialSymbol::CHROME_READER_MODE)
                    .extent(Length::px(GLYPH))
                    .color(palette().accent),
            )])
            .priority(ToolPriority::Pinned),
        )
        .group(
            tool_group([tool_item(
                segmented(typeface.unwrap_or(0))
                    .segments(typefaces.clone())
                    .control_size(ControlSize::Small)
                    .focusless()
                    .on_select(pick_typeface),
            )
            .overflow([menu_item(tr!("browser.reader.typeface"))
                .icon(MaterialSymbol::FONT_DOWNLOAD)
                .choices(typefaces, typeface, pick_typeface)])])
            .priority(ToolPriority::High),
        )
        .group(
            tool_group([
                tool_button(MaterialSymbol::TEXT_DECREASE, tr!("browser.reader.smaller"))
                    .enabled(style.size > FontSize::SMALLEST)
                    .on_press(move |shell: &mut Shell| shell.restyle_reader(smaller)),
                tool_button(MaterialSymbol::TEXT_INCREASE, tr!("browser.reader.larger"))
                    .enabled(style.size < FontSize::LARGEST)
                    .on_press(move |shell: &mut Shell| shell.restyle_reader(larger)),
            ])
            .gap(Space::HAIR)
            .priority(ToolPriority::High),
        )
        .group(tool_group([tool_item(
            segmented(width.unwrap_or(1))
                .glyphs([
                    MaterialSymbol::WIDTH_NORMAL,
                    MaterialSymbol::WIDTH_WIDE,
                    MaterialSymbol::WIDTH_FULL,
                ])
                .segments(widths.clone())
                .control_size(ControlSize::Small)
                .focusless()
                .on_select(pick_width),
        )
        .overflow([menu_item(tr!("browser.reader.width"))
            .icon(MaterialSymbol::WIDTH_NORMAL)
            .choices(widths, width, pick_width)])]))
        .group(tool_group([tool_item(
            segmented(spacing.unwrap_or(1))
                .glyphs([
                    MaterialSymbol::DENSITY_SMALL,
                    MaterialSymbol::DENSITY_MEDIUM,
                    MaterialSymbol::FORMAT_LINE_SPACING,
                ])
                .segments(spacings.clone())
                .control_size(ControlSize::Small)
                .focusless()
                .on_select(pick_spacing),
        )
        .overflow([menu_item(tr!("browser.reader.spacing"))
            .icon(MaterialSymbol::FORMAT_LINE_SPACING)
            .choices(spacings, spacing, pick_spacing)])]))
        .group(tool_group([tool_item(
            segmented(theme.unwrap_or(0))
                .segments(themes.clone())
                .control_size(ControlSize::Small)
                .focusless()
                .on_select(pick_theme),
        )
        .overflow([menu_item(tr!("browser.reader.theme"))
            .icon(MaterialSymbol::CONTRAST)
            .choices(themes, theme, pick_theme)])]))
        .spacer()
        .group(
            tool_group([tool_item(
                button(text(tr!("browser.reader.close")))
                    .ghost()
                    .small()
                    .focusless()
                    .on_press(|shell: &mut Shell| shell.close_reader()),
            )])
            .priority(ToolPriority::Pinned),
        )
        .height(BAR)
        .any()
}
