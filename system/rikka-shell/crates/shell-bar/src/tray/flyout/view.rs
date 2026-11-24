use rikka_tray::{Entry, State, Toggle};
use rikka_types::{Color, Key, Modifiers, Point, PointerButton, Rect};
use rikka_ui::{
    Align, Anchor, AnchorEdge, Element, Inset, Label, Length, Space, Styled, TextStyled, View,
    is_exiting, palette, text, theme, tr,
};
use rikka_widgets::{MenuItem, column, container, icon, menu, menu_item, menu_separator, row};
use shell_apps::AppIcons;
use shell_kit::{presses, scrim};

use crate::dropdown_panel::{DETAIL, ROW_DEPTH, ROW_INSET, ROW_RADIUS, dropdown_panel};
use crate::tray::flyout::open::Open;
use crate::tray::flyout::state::Flyout;
use crate::tray::flyout::{ICON, MENU, MENU_WIDTH, ROW, WIDTH};
use crate::tray::look::{self, Look};

pub(crate) fn view(flyout: &Flyout) -> impl View {
    let tray = flyout.tray();
    scrim(
        column! {
            if let Some(opening) = flyout.opening().filter(|_| !is_exiting()) {
                dropdown_panel(opening, WIDTH, rows(flyout, tray), on_key)
                    .beside(menus(flyout, tray));
            }
        }
        .fill(),
        Flyout::dismiss,
    )
}

fn on_key(flyout: &mut Flyout, key: Key, _modifiers: Modifiers) -> bool {
    match (key, flyout.open().is_some()) {
        (Key::Escape, true) => flyout.close_menu(),
        (Key::Escape, false) => flyout.dismiss(),
        _ => return false,
    }
    true
}

fn rows(flyout: &Flyout, tray: &State) -> Vec<Element> {
    match tray.items.is_empty() {
        true => vec![empty().build()],
        false => (0..tray.items.len())
            .filter_map(|index| Some(line(flyout, index, flyout.look(index)?)))
            .collect(),
    }
}

fn empty() -> impl View {
    container(text(tr!("shell.tray.empty")).label().muted())
        .padding(Inset::xy(ROW_INSET, ROW_DEPTH))
}

fn line(flyout: &Flyout, index: usize, look: &Look) -> Element {
    let palette = palette();
    let hovered = flyout.hovered() == Some(index);
    let attention = flyout.tray().items[index].status.needs_attention();

    let lines = column! {
        text(look.label.as_str())
            .label()
            .color(match attention {
                true => palette.accent,
                false => palette.text,
            })
            .no_wrap();
        if let Some(detail) = &look.detail {
            text(detail.as_str())
                .text_size(DETAIL)
                .muted()
                .no_wrap();
        }
    }
    .gap(Space::HAIR);

    let body = row! {
        icon(look.icon.clone())
            .extent(Length::px(ICON))
            .color(palette.text);
        lines.grow(1.0)
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width();

    let styled = container(body)
        .background(match hovered {
            true => theme().hovered(palette.surface),
            false => Color::TRANSPARENT,
        })
        .radius(ROW_RADIUS)
        .padding(Inset::xy(ROW_INSET, ROW_DEPTH))
        .fill_width()
        .on_hover(move |flyout: &mut Flyout, over: bool| flyout.hover(over.then_some(index)));

    presses(styled)
        .on_press(
            move |flyout: &mut Flyout, button: PointerButton, at: Point| {
                flyout.press(index, button, at);
                true
            },
        )
        .on_scroll(move |flyout: &mut Flyout, detents| flyout.scroll(index, detents))
        .build()
        .keyed((ROW, index))
}

fn menus(flyout: &Flyout, tray: &State) -> Vec<Element> {
    let Some((open, menu)) = flyout.open().zip(tray.menu.as_ref()) else {
        return Vec::new();
    };
    if menu.item != open.item {
        return Vec::new();
    }
    open.levels(menu)
        .into_iter()
        .enumerate()
        .map(|(level, entries)| panel(flyout, open, level, entries))
        .collect()
}

fn panel(flyout: &Flyout, open: &Open, level: usize, entries: &[Entry]) -> Element {
    let rows: Vec<MenuItem> = entries
        .iter()
        .map(|entry| command(entry, flyout.icons()))
        .collect();

    let mut panel = menu()
        .items(rows)
        .width(MENU_WIDTH)
        .on_activate(move |flyout: &mut Flyout, row| flyout.chose(level, row))
        .on_highlight(move |flyout: &mut Flyout, _: Option<usize>| flyout.highlight(level))
        .on_open(move |flyout: &mut Flyout, row, rect: Rect| {
            flyout.unfold_under(level, row, rect.y());
        });
    if level == 0 {
        panel = panel.on_dismiss(Flyout::close_menu);
    }

    panel
        .build()
        .keyed((MENU, level))
        .anchored(anchor(open, level))
}

fn anchor(open: &Open, level: usize) -> Anchor {
    match level.checked_sub(1) {
        None => Anchor::new((ROW, open.row), AnchorEdge::BesideTop),
        Some(above) => Anchor::new((MENU, above), AnchorEdge::BesideTop)
            .offset(0.0, open.opened_at(level).unwrap_or_default()),
    }
}

fn command(entry: &Entry, icons: &AppIcons) -> MenuItem {
    let Entry::Row(row) = entry else {
        return menu_separator();
    };
    let mut command = menu_item(Label::from(row.label.as_str()))
        .enabled(row.enabled)
        .submenu(row.opens_submenu())
        .selected(row.toggle.is_some_and(Toggle::is_on));
    if let Some(artwork) = look::artwork(row.icon.as_ref(), icons) {
        command = command.icon(artwork);
    }
    command
}
