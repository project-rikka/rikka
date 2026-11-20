use rikka_llm_accounts::{ModelOptions, effort_label};
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Align, Anchor, Element, Inset, Justify, Label, Length, MaterialSymbol, Space, Styled,
    TextAlign, TextDocument, TextStyled, View, palette, text, tr,
};
use rikka_widgets::{
    ControlSize, ImageFit, TextEditor, button, column, combo, container, dropdown, icon, image,
    menu, row, rule, rule_upright, spacer, spinner, text_editor, text_field,
};
use shell_kit::floating;

use crate::error::Error;
use crate::profile::Profile;
use crate::side::Side;
use crate::window::{LanguageInput, Translate};
use crate::{model, picker};

const WIDTH: f32 = 900.0;
const DOWN: f32 = 12.0;

const BORDER: f32 = 1.0;

const EDGE: f32 = 14.0;
pub(crate) const GAP: f32 = 10.0;

const PANE: f32 = 300.0;
const PREVIEW: f32 = 150.0;

const BODY: f32 = 15.0;
const LINE: f32 = 22.0;

const ICON: f32 = 20.0;
const GLYPH: f32 = 16.0;
const PICKER: f32 = 210.0;
const MODELS: f32 = 220.0;
const EFFORTS: f32 = 130.0;
const PROFILES: f32 = 150.0;

const DROP: f32 = 6.0;

const WRITTEN: &str = "translate.written";
const READ: &str = "translate.read";
const SAID: &str = "translate.said";
const MODEL_TRIGGER: &str = "translate.model";
const MODEL_PANEL: &str = "translate.model-panel";

pub(crate) fn view(translate: &Translate) -> impl View {
    floating(translate.spot(), window(translate), Translate::dismiss)
        .on_key(on_key)
        .width(WIDTH)
        .down(DOWN / 100.0)
}
fn on_key(translate: &mut Translate, key: Key, modifiers: Modifiers) -> bool {
    match key {
        Key::Escape => translate.dismiss(),
        Key::Enter if modifiers.control => translate.translate(),
        _ => return false,
    }
    true
}

fn window(translate: &Translate) -> impl View {
    let palette = palette();
    let picks = translate.picks();

    let window = column! {
        heading(translate);
        rule();
        panes(translate);
        rule();
        footing(translate, &picks);
        if translate.is_picking() {
            models_panel(&picks);
        }
    }
    .fill_width();

    container(window)
        .background(palette.background)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .resizing()
}

fn heading(translate: &Translate) -> impl View {
    row! {
        side(translate, Side::Source);
        swap(translate);
        side(translate, Side::Target);
        profile(translate);
        spacer().grow(1.0);
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .on_press(Translate::dismiss)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(EDGE, GAP))
    .fill_width()
}

fn side(translate: &Translate, side: Side) -> Element {
    let current = match side {
        Side::Source => translate.source(),
        Side::Target => translate.target(),
    };
    if let Some(language_input) = translate
        .language_input()
        .filter(|language_input| language_input.side == side)
    {
        return naming_field(language_input).build();
    }

    let offered = picker::offered(side, current);
    combo(picker::at(&offered, current))
        .options(picker::labels(&offered))
        .control_size(ControlSize::Small)
        .width(PICKER)
        .on_select(move |translate: &mut Translate, index: usize| translate.chose(side, index))
        .build()
}

fn naming_field(language_input: &LanguageInput) -> impl View {
    text_field(&language_input.typed)
        .placeholder(tr!("shell.translate.other"))
        .control_size(ControlSize::Small)
        .autofocus(true)
        .width(PICKER)
        .on_change(|| {})
        .on_submit(Translate::named)
}

fn swap(translate: &Translate) -> impl View {
    button(icon(MaterialSymbol::SWAP_HORIZ).extent(Length::px(ICON)))
        .ghost()
        .control_size(ControlSize::Small)
        .icon_only()
        .enabled(translate.swappable())
        .on_press(Translate::swap)
}

fn profile(translate: &Translate) -> impl View {
    combo(translate.profile().at())
        .options(Profile::labels())
        .control_size(ControlSize::Small)
        .width(PROFILES)
        .enabled(translate.picture().is_none())
        .on_select(|translate: &mut Translate, index: usize| translate.choose_profile(index))
}

fn panes(translate: &Translate) -> impl View {
    row! {
        pane(input(translate));
        rule_upright();
        pane(translation_pane(translate))
    }
    .height(PANE)
    .fill_width()
}

fn pane(inside: impl View) -> impl View {
    container(inside)
        .padding(Space::NORMAL)
        .fill_height()
        .grow(1.0)
}

fn input(translate: &Translate) -> Element {
    let Some(picture) = translate.picture() else {
        return prose(translate.input())
            .placeholder(tr!("shell.translate.placeholder"))
            .autofocus(true)
            .on_change(Translate::typed)
            .on_key(
                |translate: &mut Translate, key: Key, modifiers: Modifiers| {
                    let send_now = key == Key::Enter && modifiers.control;
                    if send_now {
                        translate.translate();
                    }
                    send_now
                },
            )
            .build()
            .keyed(WRITTEN);
    };

    column! {
        image(picture.shown.clone())
            .fit(ImageFit::ScaleDown)
            .max_height(PREVIEW)
            .fill_width();
        if !translate.read().is_empty() {
            prose(translate.read()).read_only(true).build().keyed(READ);
        }
    }
    .gap(Space::SNUG)
    .fill_height()
    .fill_width()
    .build()
}

fn prose(document: &TextDocument) -> TextEditor {
    text_editor(document)
        .gutter(false)
        .font_size(BODY)
        .line_height(LINE)
        .grow(1.0)
}

fn translation_pane(translate: &Translate) -> Element {
    if let Some(trouble) = translate.trouble() {
        return show_error(trouble).build();
    }
    if translate.is_working() && translate.translation().is_empty() {
        return column! {
            spinner().extent(Length::px(ICON)).color(palette().accent)
        }
        .align_items(Align::Center)
        .justify(Justify::Center)
        .fill_height()
        .fill_width()
        .build();
    }

    let shown = prose(translate.translation())
        .placeholder(tr!("shell.translate.awaiting"))
        .read_only(true);
    let shown = match translate.is_stale() {
        true => shown.color(palette().text_muted),
        false => shown,
    };
    shown.build().keyed(SAID)
}

fn show_error(trouble: &Error) -> impl View {
    column! {
        icon(MaterialSymbol::ERROR)
            .extent(Length::px(ICON))
            .color(palette().danger);
        text(spoken(trouble))
            .caption()
            .muted()
            .align(TextAlign::Center)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .justify(Justify::Center)
    .fill_height()
    .fill_width()
}

fn spoken(trouble: &Error) -> Label {
    match trouble {
        Error::Unconfigured => tr!("shell.translate.unconfigured"),
        Error::Blind { model } => tr!("shell.translate.blind", model = model),
        Error::TooMuch {
            length: input,
            most,
        } => tr!(
            "shell.translate.too-much",
            input = *input as i64,
            most = *most as i64
        ),
        refused => Label::from(refused.to_string()),
    }
}

fn footing(translate: &Translate, picks: &ModelOptions) -> impl View {
    row! {
        counter(translate);
        spacer().grow(1.0);
        models(picks, translate.is_picking());
        efforts(picks);
        button(icon(MaterialSymbol::DELETE_SWEEP).extent(Length::px(ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(!translate.is_empty())
            .on_press(Translate::clear);
        button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(ICON)))
            .ghost()
            .control_size(ControlSize::Small)
            .icon_only()
            .enabled(!translate.translation().is_empty())
            .on_press(|translate: &mut Translate| translate.copy());
        asking(translate)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(EDGE, GAP))
    .fill_width()
}

fn asking(translate: &Translate) -> Element {
    if translate.is_working() {
        return button(labelled(
            MaterialSymbol::STOP_CIRCLE,
            tr!("shell.translate.stop"),
        ))
        .secondary()
        .control_size(ControlSize::Small)
        .on_press(Translate::stop)
        .build();
    }
    button(labelled(
        MaterialSymbol::TRANSLATE,
        tr!("shell.translate.run"),
    ))
    .primary()
    .control_size(ControlSize::Small)
    .enabled(translate.can_translate())
    .on_press(Translate::translate)
    .build()
}

fn labelled(symbol: MaterialSymbol, label: Label) -> impl View {
    row! {
        icon(symbol).extent(Length::px(GLYPH));
        text(label)
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn counter(translate: &Translate) -> Option<impl View> {
    if translate.picture().is_some() {
        return None;
    }
    let most = translate.most_characters()?;
    let input = translate.characters();
    let palette = palette();
    let ink = match input > most {
        true => palette.danger,
        false => palette.text_muted,
    };
    Some(text(format!("{input} / {most}")).caption().color(ink))
}

fn models(picks: &ModelOptions, picking: bool) -> Element {
    dropdown(picks.selected_index())
        .options(model::names(picks))
        .placeholder(tr!("shell.translate.model"))
        .control_size(ControlSize::Small)
        .width(MODELS)
        .open(picking)
        .enabled(!picks.is_empty())
        .on_toggle(|translate: &mut Translate, open: bool| translate.pick(open))
        .build()
        .keyed(MODEL_TRIGGER)
}

fn models_panel(picks: &ModelOptions) -> Element {
    menu()
        .items(model::rows(picks))
        .control_size(ControlSize::Small)
        .on_activate(|translate: &mut Translate, row: usize| translate.model(row))
        .on_dismiss(|translate: &mut Translate| translate.pick(false))
        .build()
        .keyed(MODEL_PANEL)
        .anchored(
            Anchor::above(MODEL_TRIGGER)
                .offset(0.0, DROP)
                .match_target_width(),
        )
}

fn efforts(picks: &ModelOptions) -> Element {
    let offered = picks.efforts();
    if offered.is_empty() {
        return spacer().width(0.0).grow(0.0).build();
    }
    combo(picks.selected_effort_index())
        .options(offered.iter().copied().map(effort_label))
        .placeholder(tr!("shell.translate.effort.name"))
        .control_size(ControlSize::Small)
        .width(EFFORTS)
        .on_select(|translate: &mut Translate, index: usize| translate.effort(index))
        .build()
}
