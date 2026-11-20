use proton_session::{ErrorBanner, Failure};
use proton_vaults::{Vault, colors, icons};
use rikka_app::prelude::*;
use rikka_proton_pass::ShareId;
use rikka_widgets::{column, row};

use crate::pane;
use crate::pass::Pass;
use crate::pass::marks::{self, SHADES, SYMBOLS};

const PREVIEW: f32 = 48.0;
const SWATCH: f32 = 32.0;
const MARK: f32 = 36.0;
const PER_ROW: usize = 10;

#[derive(Debug, State)]
pub(crate) struct VaultForm {
    share: Option<ShareId>,
    name: TextDocument,
    description: String,
    icon: usize,
    color: usize,
    busy: bool,
    error: ErrorBanner,
}

impl VaultForm {
    pub(crate) fn fresh() -> VaultForm {
        VaultForm {
            share: None,
            name: TextDocument::new(""),
            description: String::new(),
            icon: 0,
            color: 0,
            busy: false,
            error: ErrorBanner::default(),
        }
    }

    pub(crate) fn from_vault(vault: &Vault) -> VaultForm {
        VaultForm {
            share: Some(vault.share.clone()),
            name: TextDocument::new(&vault.name),
            description: vault.description.clone(),
            icon: icons()
                .iter()
                .position(|icon| *icon == vault.icon)
                .unwrap_or(0),
            color: colors()
                .iter()
                .position(|color| *color == vault.color)
                .unwrap_or(0),
            busy: false,
            error: ErrorBanner::default(),
        }
    }

    pub(crate) fn share(&self) -> Option<&ShareId> {
        self.share.as_ref()
    }

    pub(crate) fn named(&self) -> String {
        self.name.text().trim().to_owned()
    }

    pub(crate) fn description(&self) -> String {
        self.description.clone()
    }

    pub(crate) fn icon(&self) -> i32 {
        icons().get(self.icon).copied().unwrap_or_default()
    }

    pub(crate) fn color(&self) -> i32 {
        colors().get(self.color).copied().unwrap_or_default()
    }

    pub(crate) fn choose_icon(&mut self, at: usize) {
        self.icon = at;
    }

    pub(crate) fn choose_color(&mut self, at: usize) {
        self.color = at;
    }

    pub(crate) fn working(&mut self) {
        self.busy = true;
        self.error.clear();
    }

    pub(crate) fn failed(&mut self, failure: Failure) {
        self.busy = false;
        self.error.set(failure);
    }
}

pub(crate) fn view(form: &VaultForm) -> Element {
    let tint = marks::shade(form.color);

    column! {
        pane::toolbar(
            row! {
                text(match form.share.is_some() {
                    true => tr!("proton.pass.vault-editing"),
                    false => tr!("proton.pass.vault-new"),
                })
                .label()
                .semibold()
                .grow(1.0);
                button(text(match form.share.is_some() {
                    true => tr!("proton.pass.save"),
                    false => tr!("proton.pass.vault-create"),
                }))
                .primary()
                .small()
                .loading(form.busy)
                .on_press(|vault: &mut Pass| vault.keep_vault());
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|vault: &mut Pass| vault.close_vault_form());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        );
        rule();
        pane::error_banner(&form.error, |form: &mut VaultForm| &mut form.error);
        scrollable(
            column! {
                row! {
                    marks::badge(SYMBOLS[form.icon.min(SYMBOLS.len() - 1)], tint, PREVIEW);
                    pane::input(
                        tr!("proton.pass.vault-name"),
                        text_field(&form.name)
                            .placeholder(tr!("proton.pass.vault-untitled"))
                            .autofocus(true)
                            .on_submit(|vault: &mut Pass| vault.keep_vault())
                            .fill_width(),
                    )
                    .grow(1.0);
                }
                .gap(Space::NORMAL)
                .align_items(Align::End)
                .fill_width();
                column! {
                    pane::heading(tr!("proton.pass.vault-colour"));
                    swatches(form);
                }
                .gap(Space::SNUG)
                .fill_width();
                column! {
                    pane::heading(tr!("proton.pass.vault-mark"));
                    symbols(form, tint);
                }
                .gap(Space::SNUG)
                .fill_width();
                if form.share.is_some() {
                    row! {
                        button(pane::labelled(MaterialSymbol::DELETE, tr!("proton.pass.vault-drop")))
                            .danger()
                            .on_press(|vault: &mut Pass| vault.drop_vault());
                        spacer().grow(1.0);
                    }
                    .fill_width();
                }
            }
            .gap(Space::ROOMY)
            .padding(Space::ROOMY)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .fill()
    .build()
}

fn swatches(form: &VaultForm) -> impl View {
    let chosen = form.color;

    rows((0..SHADES).map(move |at| {
        let tint = marks::shade(at);
        container(
            container(spacer())
                .background(tint)
                .radius(SWATCH * 0.35)
                .size(SWATCH * 0.7, SWATCH * 0.7),
        )
        .background(match at == chosen {
            true => tint.with_alpha(0.3),
            false => Color::TRANSPARENT,
        })
        .radius(SWATCH / 2.0)
        .size(SWATCH, SWATCH)
        .center()
        .on_press(move |form: &mut VaultForm| form.choose_color(at))
        .build()
    }))
}

fn symbols(form: &VaultForm, tint: Color) -> impl View {
    let chosen = form.icon;

    rows(SYMBOLS.into_iter().enumerate().map(move |(at, symbol)| {
        let picked = at == chosen;
        container(icon(symbol).extent(Length::px(18.0)).color(match picked {
            true => tint,
            false => palette().text,
        }))
        .background(match picked {
            true => tint.with_alpha(0.16),
            false => palette().surface_variant,
        })
        .border(
            2.0,
            match picked {
                true => tint,
                false => Color::TRANSPARENT,
            },
        )
        .radius(MARK / 2.0)
        .size(MARK, MARK)
        .center()
        .on_press(move |form: &mut VaultForm| form.choose_icon(at))
        .build()
    }))
}

fn rows(cells: impl Iterator<Item = Element>) -> impl View {
    let mut cells = cells.peekable();
    let mut rows = Vec::new();
    while cells.peek().is_some() {
        rows.push(
            row_of(cells.by_ref().take(PER_ROW))
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .build(),
        );
    }
    column_of(rows).gap(Space::SNUG)
}
