use code_meaning::{Caution, Link, Meaning};
use rikka_network::{Invitation, Security, Unjoinable};
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers, Rect};
use rikka_ui::{
    Align, Anchor, Element, Inset, Label, Length, MaterialSymbol, Space, Styled, TextColor,
    TextStyled, View, local_state, palette, text, tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, button, column, column_of, container, icon, row, scrollable, spacer,
};
use shell_kit::{CURSOR, mark, sheet};

use crate::scan::acting::Act;
use crate::scan::codes::Code;
use crate::selector::Selector;

const WIDTH: f32 = 360.0;
pub(crate) const GAP: f32 = 10.0;
const HAIR: f32 = 1.0;
const ICON: f32 = 20.0;
const SMALL_ICON: f32 = 16.0;
const ADDRESS: f32 = 72.0;
const WRITTEN: f32 = 168.0;
const SIZE: ControlSize = ControlSize::Small;
const HIDDEN: char = '•';

pub(crate) fn beside(area: Rect, panel: impl View) -> impl View {
    column! {
        mark(area);
        sheet(panel, on_key)
            .build()
            .width(WIDTH)
            .anchored(Anchor::below(CURSOR).offset(0.0, GAP))
    }
}

fn on_key(selector: &mut Selector, key: Key, _modifiers: Modifiers) -> bool {
    match key {
        Key::Escape => {
            selector.point(None);
            true
        }
        _ => false,
    }
}

pub(crate) fn card(index: usize, code: &Code, trouble: Option<Unjoinable>) -> Element {
    let body = match &code.meaning {
        Meaning::Link(link) => link_body(link),
        Meaning::Network(invitation) => network_body(invitation, trouble),
        Meaning::Text => text_body(&code.text),
    };
    panel(
        column! {
            heading(kind_symbol(code), kind_label(&code.meaning), format_name(code));
            body;
            actions(index, &code.meaning)
        }
        .gap(Space::TIGHT),
    )
    .build()
    .keyed(index)
}

pub(crate) fn missed() -> impl View {
    panel(
        row! {
            icon(MaterialSymbol::SEARCH_OFF)
                .extent(Length::px(ICON))
                .color(palette().text_muted);
            text(tr!("shell.capture.scan.missed")).grow(1.0);
            close()
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
}

fn panel(content: impl View) -> impl View {
    container(content)
        .surface()
        .radius(Corner::Panel)
        .border(HAIR, palette().outline)
        .pane()
        .padding(Inset::all(GAP))
        .fill_width()
}

fn heading(symbol: MaterialSymbol, kind: Label, format: Label) -> impl View {
    row! {
        icon(symbol).extent(Length::px(ICON)).color(palette().accent);
        text(kind).label();
        spacer().grow(1.0);
        text(format).caption().muted().no_wrap();
        close()
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn close() -> impl View {
    button(icon(MaterialSymbol::CLOSE).extent(Length::px(SMALL_ICON)))
        .ghost()
        .control_size(SIZE)
        .icon_only()
        .on_press(|selector: &mut Selector| selector.point(None))
}

fn link_body(link: &Link) -> Element {
    let cautions = link
        .cautions()
        .iter()
        .map(|caution| warned(caution_label(*caution)).build());
    column! {
        text(link.host().to_owned()).heading();
        scrollable(text(link.address().to_owned()).caption().muted())
            .max_height(Length::px(ADDRESS));
        column_of(cautions).gap(Space::TIGHT)
    }
    .gap(Space::TIGHT)
    .build()
}

fn network_body(invitation: &Invitation, trouble: Option<Unjoinable>) -> Element {
    let mut facts = vec![security_label(invitation.security)];
    if invitation.hidden {
        facts.push(tr!("shell.capture.scan.hidden"));
    }
    let facts = facts
        .iter()
        .map(|fact| fact.as_str().to_owned())
        .collect::<Vec<_>>()
        .join(" · ");
    column! {
        text(invitation.ssid.name().into_owned()).heading();
        text(facts).caption().muted();
        (!invitation.passphrase.is_empty()).then(|| passphrase(invitation.passphrase.clone()));
        trouble.map(|trouble| warned(unjoinable_label(trouble)).build())
    }
    .gap(Space::TIGHT)
    .build()
}

fn passphrase(secret: String) -> Element {
    local_state(false, move |shown: bool, set| {
        let shown_text = match shown {
            true => secret.clone(),
            false => HIDDEN
                .to_string()
                .repeat(secret.chars().count().clamp(8, 16)),
        };
        let symbol = match shown {
            true => MaterialSymbol::VISIBILITY_OFF,
            false => MaterialSymbol::VISIBILITY,
        };
        row! {
            text(tr!("shell.capture.scan.password")).caption().muted();
            text(shown_text).body().grow(1.0);
            button(icon(symbol).extent(Length::px(SMALL_ICON)))
                .ghost()
                .control_size(SIZE)
                .icon_only()
                .on_press(move || set(!shown))
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
    })
}

fn text_body(body_text: &str) -> Element {
    scrollable(text(body_text.to_owned()).body())
        .max_height(Length::px(WRITTEN))
        .build()
}

fn actions(index: usize, meaning: &Meaning) -> impl View {
    let primary = Act::primary(meaning);
    let copy = (primary != Act::Copy).then(|| copy_label(meaning));
    row! {
        spacer().grow(1.0);
        copy.map(|label| act_button(index, Act::Copy, label, ButtonVariant::Secondary));
        act_button(index, primary, act_label(primary), ButtonVariant::Primary)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn act_button(index: usize, act: Act, label: Label, variant: ButtonVariant) -> impl View {
    button(
        row! {
            icon(act_symbol(act)).extent(Length::px(SMALL_ICON));
            text(label).no_wrap()
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .variant(variant)
    .control_size(SIZE)
    .on_press(move |selector: &mut Selector| selector.act(index, act))
}

fn act_symbol(act: Act) -> MaterialSymbol {
    match act {
        Act::Open => MaterialSymbol::OPEN_IN_NEW,
        Act::Copy => MaterialSymbol::CONTENT_COPY,
        Act::Join => MaterialSymbol::WIFI,
        Act::Settings => MaterialSymbol::SETTINGS,
    }
}

fn act_label(act: Act) -> Label {
    match act {
        Act::Open => tr!("shell.capture.scan.open"),
        Act::Copy => tr!("shell.capture.scan.copy"),
        Act::Join => tr!("shell.capture.scan.join"),
        Act::Settings => tr!("shell.capture.scan.settings"),
    }
}

fn copy_label(meaning: &Meaning) -> Label {
    match meaning {
        Meaning::Network(invitation) if invitation.passphrase.is_empty() => {
            tr!("shell.capture.scan.copy-name")
        }
        Meaning::Network(_) => tr!("shell.capture.scan.copy-password"),
        Meaning::Link(_) | Meaning::Text => tr!("shell.capture.scan.copy"),
    }
}

fn warned(label: Label) -> impl View {
    row! {
        icon(MaterialSymbol::WARNING)
            .extent(Length::px(SMALL_ICON))
            .color(palette().warning);
        text(label).caption().color(TextColor::Warning).grow(1.0)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Start)
}

fn kind_symbol(code: &Code) -> MaterialSymbol {
    match &code.meaning {
        Meaning::Link(_) => MaterialSymbol::LINK,
        Meaning::Network(_) => MaterialSymbol::WIFI,
        Meaning::Text if code.format.is_linear() => MaterialSymbol::BARCODE,
        Meaning::Text => MaterialSymbol::QR_CODE,
    }
}

fn kind_label(meaning: &Meaning) -> Label {
    match meaning {
        Meaning::Link(_) => tr!("shell.capture.scan.kind.link"),
        Meaning::Network(_) => tr!("shell.capture.scan.kind.network"),
        Meaning::Text => tr!("shell.capture.scan.kind.text"),
    }
}

fn format_name(code: &Code) -> Label {
    match code.format.name() {
        Some(name) => Label::from(name),
        None => tr!("shell.capture.scan.barcode"),
    }
}

fn security_label(security: Security) -> Label {
    match security {
        Security::Open | Security::Enhanced => tr!("shell.capture.scan.security.open"),
        Security::Wep => tr!("shell.capture.scan.security.wep"),
        Security::Personal => tr!("shell.capture.scan.security.personal"),
        Security::Wpa3Personal => tr!("shell.capture.scan.security.wpa3"),
        Security::Enterprise | Security::Wpa3Enterprise => {
            tr!("shell.capture.scan.security.enterprise")
        }
    }
}

fn caution_label(caution: Caution) -> Label {
    match caution {
        Caution::Unencrypted => tr!("shell.capture.scan.caution.unencrypted"),
        Caution::ForeignLetters => tr!("shell.capture.scan.caution.foreign-letters"),
        Caution::Credentials => tr!("shell.capture.scan.caution.credentials"),
        Caution::NumericHost => tr!("shell.capture.scan.caution.numeric-host"),
    }
}

fn unjoinable_label(trouble: Unjoinable) -> Label {
    match trouble {
        Unjoinable::NoRadio => tr!("shell.capture.scan.unjoinable.no-radio"),
        Unjoinable::RadioOff => tr!("shell.capture.scan.unjoinable.radio-off"),
        Unjoinable::NeedsAccount => tr!("shell.capture.scan.unjoinable.needs-account"),
        Unjoinable::OutOfRange => tr!("shell.capture.scan.unjoinable.out-of-range"),
    }
}
