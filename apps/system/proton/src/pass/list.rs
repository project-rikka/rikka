use std::sync::Arc;

use proton_vaults::{Entry, ItemScope, ItemType, SortKey};
use rikka_app::prelude::*;
use rikka_proton_pass::{ShareId, WifiSecurity};
use rikka_widgets::{column, row};

use crate::pass::{Pass, Popup, marks, rows};
use crate::{pane, wording};

const ROW: f32 = 60.0;
const SORT: &str = "proton-pass-sort";
const NEW: &str = "proton-pass-new";

const OFFERED: [Option<ItemType>; 9] = [
    Some(ItemType::Login),
    Some(ItemType::Alias),
    Some(ItemType::Note),
    Some(ItemType::Card),
    Some(ItemType::Identity),
    None,
    Some(ItemType::Wifi),
    Some(ItemType::SshKey),
    Some(ItemType::Custom),
];

pub(crate) fn vaults(vault: &Pass) -> impl View {
    let chosen = vault.vaults().selected().cloned();
    let scope = vault.vaults().scope();

    column! {
        container(
            row! {
                button(pane::labelled(MaterialSymbol::ADD, tr!("proton.pass.new-item")))
                    .primary()
                    .small()
                    .enabled(!vault.vaults().vaults().is_empty())
                    .on_press(|vault: &mut Pass| vault.popped(Some(Popup::New)))
                    .grow(1.0)
                    .build()
                    .keyed(NEW);
                button(icon(MaterialSymbol::ADD_MODERATOR).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|vault: &mut Pass| vault.write_vault());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::xy(12.0, 12.0))
        .fill_width();
        scrollable(
            column! {
                vault_leaf(
                    tr!("proton.pass.all-vaults"),
                    (MaterialSymbol::SHIELD, palette().accent),
                    vault.vaults().all(),
                    chosen.is_none() && scope == ItemScope::All,
                    None,
                );
                column_of(vault.vaults().vaults().iter().map(|item_vault| {
                    vault_leaf(
                        Label::from(item_vault.name.as_str()),
                        (marks::symbol_of(item_vault.icon), marks::shade_of(item_vault.color)),
                        vault.vaults().count(&item_vault.share),
                        chosen.as_ref() == Some(&item_vault.share),
                        Some(item_vault.share.clone()),
                    )
                    .build()
                }))
                .gap(Space::HAIR)
                .fill_width();
                rule();
                trashed(vault.vaults().trashed(), scope == ItemScope::Trash);
            }
            .gap(Space::HAIR)
            .padding(Inset::xy(8.0, 0.0))
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .width(pane::SIDEBAR)
    .fill_height()
}

pub(crate) fn creations() -> Element {
    menu()
        .items(OFFERED.iter().map(|offered| {
            match *offered {
                Some(item_type) => menu_item(named(item_type))
                    .icon(symbol(item_type))
                    .on_press(move |vault: &mut Pass| {
                        vault.popped(None);
                        vault.write(item_type);
                    }),
                None => menu_separator(),
            }
        }))
        .on_dismiss(|vault: &mut Pass| vault.popped(None))
        .build()
        .anchored(Anchor::below(NEW).offset(0.0, 4.0))
}

fn leaf<Marker>(
    name: Label,
    (symbol, tint): (MaterialSymbol, Color),
    chosen: bool,
    trailing: impl View,
    press: impl rikka_ui::IntoHandler<(), (), Marker>,
) -> Container {
    container(
        row! {
            marks::badge(symbol, tint, 26.0);
            text(name)
                .label()
                .color(match chosen {
                    true => palette().accent,
                    false => palette().text,
                })
                .no_wrap()
                .grow(1.0);
            trailing;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .background(match chosen {
        true => palette().accent.with_alpha(0.12),
        false => Color::TRANSPARENT,
    })
    .radius(Corner::Item)
    .padding(Inset::xy(8.0, 5.0))
    .fill_width()
    .on_press(press)
}

fn vault_leaf(
    name: Label,
    mark: (MaterialSymbol, Color),
    count: usize,
    chosen: bool,
    share: Option<ShareId>,
) -> Container {
    let target = share.clone();

    leaf(
        name,
        mark,
        chosen,
        row! {
            if let Some(share) = target {
                button(icon(MaterialSymbol::EDIT).extent(Length::px(13.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |vault: &mut Pass| vault.edit_vault(&share));
            }
            text(wording::count(count)).caption().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
        move |vault: &mut Pass| vault.choose(share.clone()),
    )
}

fn trashed(count: usize, chosen: bool) -> Container {
    leaf(
        tr!("proton.pass.trash"),
        (MaterialSymbol::DELETE, palette().text_muted),
        chosen,
        text(wording::count(count)).caption().muted().no_wrap(),
        move |vault: &mut Pass| {
            vault.show(match chosen {
                true => ItemScope::All,
                false => ItemScope::Trash,
            });
        },
    )
}

pub(crate) fn toolbar(vault: &Pass) -> impl View {
    pane::toolbar(
        row! {
            text_field(vault.search())
                .placeholder(tr!("proton.pass.search"))
                .claims_focus(vault.seeking())
                .on_blur(Pass::sought)
                .control_size(ControlSize::Small)
                .on_change(|vault: &mut Pass| vault.look())
                .grow(1.0);
            button(
                row! {
                    icon(MaterialSymbol::SWAP_VERT).extent(Length::px(15.0));
                    text(sorting(vault.vaults().sorted())).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(|vault: &mut Pass| vault.popped(Some(Popup::Sort)))
            .build()
            .keyed(SORT);
            button(icon(MaterialSymbol::REFRESH).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .loading(vault.is_busy())
                .on_press(|vault: &mut Pass| vault.refresh());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

pub(crate) fn sorts(chosen: SortKey) -> Element {
    menu()
        .items(SortKey::ALL.iter().map(|&key| {
            menu_item(sorting(key))
                .selected(key == chosen)
                .on_press(move |vault: &mut Pass| vault.sort(key))
        }))
        .on_dismiss(|vault: &mut Pass| vault.popped(None))
        .build()
        .anchored(Anchor::below(SORT).offset(0.0, 4.0))
}

pub(crate) fn view(vault: &Pass) -> Element {
    let entries = Arc::clone(vault.vaults().shown());

    if entries.is_empty() {
        return match vault.is_busy() {
            true => pane::working(tr!("proton.pass.reading")).build(),
            false => pane::nothing(MaterialSymbol::SHIELD_LOCK, tr!("proton.pass.empty")).build(),
        };
    }

    let selected = vault.chosen().and_then(|(share, id)| {
        entries
            .iter()
            .position(|entry| &entry.share == share && &entry.id == id)
    });

    lazy_column(entries.len(), move |slot| {
        cell(&entries[slot.index], selected == Some(slot.index)).height(ROW)
    })
    .uniform(ROW)
    .selected(selected)
    .place(vault.place())
    .on_select(|vault: &mut Pass, at: usize| vault.select(at))
    .grow(1.0)
    .fill_width()
    .build()
}

fn cell(entry: &Entry, chosen: bool) -> Element {
    let mut cell = rows::cell(&entry.title, subtitle(entry), tint(entry.item_type)).chosen(chosen);

    if entry.pinned {
        cell = cell.mark(
            icon(MaterialSymbol::PUSH_PIN)
                .extent(Length::px(12.0))
                .color(palette().text_muted),
        );
    }
    if entry.has_code {
        cell = cell.mark(
            icon(MaterialSymbol::TIMER)
                .extent(Length::px(12.0))
                .color(palette().text_muted),
        );
    }

    cell.into()
}

pub(crate) fn subtitle(entry: &Entry) -> String {
    match entry.subtitle.is_empty() {
        true => named(entry.item_type).as_str().to_owned(),
        false => entry.subtitle.clone(),
    }
}

pub(crate) fn secured(security: i32) -> Label {
    match WifiSecurity::try_from(security) {
        Ok(WifiSecurity::Unspecified) | Err(_) => tr!("proton.pass.wifi-any"),
        Ok(security) => Label::from(security.as_str()),
    }
}

pub(crate) fn named(item_type: ItemType) -> Label {
    match item_type {
        ItemType::Login => tr!("proton.pass.kind.login"),
        ItemType::Alias => tr!("proton.pass.kind.alias"),
        ItemType::Note => tr!("proton.pass.kind.note"),
        ItemType::Card => tr!("proton.pass.kind.card"),
        ItemType::Identity => tr!("proton.pass.kind.identity"),
        ItemType::SshKey => tr!("proton.pass.kind.ssh"),
        ItemType::Wifi => tr!("proton.pass.kind.wifi"),
        ItemType::Custom => tr!("proton.pass.kind.custom"),
    }
}

pub(crate) fn symbol(item_type: ItemType) -> MaterialSymbol {
    match item_type {
        ItemType::Login => MaterialSymbol::KEY,
        ItemType::Alias => MaterialSymbol::ALTERNATE_EMAIL,
        ItemType::Note => MaterialSymbol::STICKY_NOTE_2,
        ItemType::Card => MaterialSymbol::CREDIT_CARD,
        ItemType::Identity => MaterialSymbol::BADGE,
        ItemType::SshKey => MaterialSymbol::TERMINAL,
        ItemType::Wifi => MaterialSymbol::WIFI,
        ItemType::Custom => MaterialSymbol::BOOKMARK_MANAGER,
    }
}

pub(crate) fn tint(item_type: ItemType) -> Color {
    match item_type {
        ItemType::Login => palette().accent,
        ItemType::Alias => palette().success,
        ItemType::Card => palette().warning,
        _ => palette().text_muted,
    }
}

fn sorting(sorted: SortKey) -> Label {
    match sorted {
        SortKey::Recent => tr!("proton.pass.sort.recent"),
        SortKey::TitleAscending => tr!("proton.pass.sort.title-ascending"),
        SortKey::TitleDescending => tr!("proton.pass.sort.title-descending"),
        SortKey::Oldest => tr!("proton.pass.sort.oldest"),
        SortKey::Newest => tr!("proton.pass.sort.newest"),
    }
}
