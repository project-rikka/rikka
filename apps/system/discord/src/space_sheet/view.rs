use discord_core::SpaceTemplate;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{LONGEST_NAME, SpaceSheet, Step};
use crate::brand;
use crate::chat::Chat;
use crate::desk::BODY;

const WIDTH: f32 = 440.0;
const FACE: f32 = 80.0;
const BADGE: f32 = 24.0;
const CHOICE: f32 = 60.0;
const CHOICE_ICON: f32 = 40.0;
const EXAMPLES: [&str; 3] = [
    "hTKzmak",
    "https://discord.gg/hTKzmak",
    "https://discord.gg/cool-people",
];
const STARTERS: [SpaceTemplate; 5] = [
    SpaceTemplate::SchoolClub,
    SpaceTemplate::StudyGroup,
    SpaceTemplate::Friends,
    SpaceTemplate::Creators,
    SpaceTemplate::LocalCommunity,
];

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let state = chat.space_sheet();
    let body = match state.step()? {
        Step::Choose => choose(),
        Step::Customize(template) => customize(state, template),
        Step::Join => join(state),
    };
    Some(
        sheet(
            container(body)
                .padding(Inset::all(Space::LOOSE))
                .fill_width(),
        )
        .width(WIDTH)
        .on_dismiss(|chat: &mut Chat| chat.close_space_sheet())
        .keyed("discord-space-sheet")
        .centered_on(BODY)
        .build(),
    )
}

pub(crate) fn scrim() -> Element {
    rikka_widgets::scrim()
        .on_dismiss(|chat: &mut Chat| chat.close_space_sheet())
        .transition(fade())
        .anchored(Anchor::covering(BODY))
}

fn choose() -> Element {
    column! {
        header(tr!("discord.space-sheet.create-title"), tr!("discord.space-sheet.create-hint"));
        choice(SpaceTemplate::Blank);
        text(tr!("discord.space-sheet.templates")).caption().semibold().muted().fill_width();
        column_of(STARTERS.map(choice)).gap(Space::SNUG).fill_width();
        column! {
            text(tr!("discord.space-sheet.have-invite")).heading().semibold();
            button(text(tr!("discord.space-sheet.join")))
                .secondary()
                .on_press(|state: &mut SpaceSheet| state.show(Step::Join))
                .fill_width();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(Space::NORMAL, Space::NONE, Space::NONE, Space::NONE))
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn customize(state: &SpaceSheet, template: SpaceTemplate) -> Element {
    let busy = state.is_busy();
    column! {
        header(
            tr!("discord.space-sheet.customize-title"),
            tr!("discord.space-sheet.customize-hint"),
        );
        container(icon_picker(state, busy)).center().fill_width();
        column! {
            text(tr!("discord.space-sheet.name")).caption().semibold().muted();
            text_field(state.name_field())
                .max_len(LONGEST_NAME)
                .autofocus(true)
                .enabled(!busy)
                .on_change(|state: &mut SpaceSheet| state.renamed())
                .on_submit(move |chat: &mut Chat| chat.create_space(template))
                .fill_width();
        }
        .gap(Space::TIGHT)
        .fill_width();
        if let Some(error_message) = state.error_message() {
            failure(error_message);
        }
        footer(
            busy,
            button(text(tr!("discord.space-sheet.create")))
                .primary()
                .enabled(state.is_named() && !busy)
                .on_press(move |chat: &mut Chat| chat.create_space(template))
                .any(),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn join(state: &SpaceSheet) -> Element {
    let busy = state.is_busy();
    column! {
        header(tr!("discord.space-sheet.join-title"), tr!("discord.space-sheet.join-hint"));
        column! {
            text(tr!("discord.space-sheet.invite")).caption().semibold().muted();
            text_field(state.invite_field())
                .placeholder(EXAMPLES[1])
                .autofocus(true)
                .enabled(!busy)
                .error(state.error_message().is_some())
                .on_change(|state: &mut SpaceSheet| state.retyped())
                .on_submit(|chat: &mut Chat| chat.join_space())
                .fill_width();
        }
        .gap(Space::TIGHT)
        .fill_width();
        if let Some(error_message) = state.error_message() {
            failure(error_message);
        }
        column! {
            text(tr!("discord.space-sheet.invite-examples")).caption().semibold().muted();
            column_of(EXAMPLES.map(|example| text(example).caption().any())).gap(Space::HAIR);
        }
        .gap(Space::TIGHT)
        .fill_width();
        footer(
            busy,
            button(text(tr!("discord.space-sheet.join-button")))
                .primary()
                .enabled(state.is_invited() && !busy)
                .on_press(|chat: &mut Chat| chat.join_space())
                .any(),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn header(title: Label, hint: Label) -> impl View {
    column! {
        text(title).title().bold().align(TextAlign::Center).fill_width();
        text(hint).body().muted().align(TextAlign::Center).fill_width();
    }
    .gap(Space::SNUG)
    .padding(Inset::edges(
        Space::NONE,
        Space::NONE,
        Space::SNUG,
        Space::NONE,
    ))
    .fill_width()
}

fn choice(template: SpaceTemplate) -> Element {
    container(
        row! {
            container(icon(symbol(template)).extent(Length::px(24.0)).color(Color::WHITE))
                .center()
                .background(brand::blurple())
                .radius(CHOICE_ICON / 2.0)
                .size(Length::px(CHOICE_ICON), Length::px(CHOICE_ICON));
            text(label(template)).label().semibold().no_wrap().grow(1.0);
            icon(MaterialSymbol::CHEVRON_RIGHT).extent(Length::px(20.0)).color(palette().text_muted);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .border(1.0, palette().outline)
    .radius(Corner::Control)
    .on_press(move |state: &mut SpaceSheet| state.show(Step::Customize(template)))
    .height(CHOICE)
    .fill_width()
    .any()
}

fn icon_picker(state: &SpaceSheet, busy: bool) -> Element {
    let face = match state.icon() {
        Some(png) => image(png.clone())
            .fit(ImageFit::Cover)
            .radius(FACE / 2.0)
            .size(Length::px(FACE), Length::px(FACE))
            .any(),
        None => container(
            column! {
                icon(MaterialSymbol::PHOTO_CAMERA).extent(Length::px(24.0)).color(palette().text_muted);
                text(tr!("discord.space-sheet.upload")).caption().semibold().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .align_items(Align::Center),
        )
        .center()
        .border(2.0, palette().outline)
        .radius(FACE / 2.0)
        .size(Length::px(FACE), Length::px(FACE))
        .any(),
    };
    let badge = container(
        icon(MaterialSymbol::ADD)
            .extent(Length::px(16.0))
            .color(Color::WHITE),
    )
    .center()
    .background(brand::blurple())
    .radius(BADGE / 2.0)
    .size(Length::px(BADGE), Length::px(BADGE))
    .position(FACE - BADGE, 0.0)
    .any();
    let mut picker = container(column_of([face, badge])).size(Length::px(FACE), Length::px(FACE));
    if !busy {
        picker = picker.on_press(|chat: &mut Chat| chat.choose_space_icon());
    }
    picker.any()
}

fn failure(error_message: &str) -> impl View {
    text(error_message.to_owned())
        .caption()
        .danger()
        .fill_width()
}

fn footer(busy: bool, confirm: Element) -> impl View {
    row! {
        button(text(tr!("discord.space-sheet.back")))
            .ghost()
            .enabled(!busy)
            .on_press(|state: &mut SpaceSheet| state.show(Step::Choose));
        spacer().grow(1.0);
        if busy {
            spinner();
        }
        confirm;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::SNUG,
        Space::NONE,
        Space::NONE,
        Space::NONE,
    ))
    .fill_width()
}

fn label(template: SpaceTemplate) -> Label {
    match template {
        SpaceTemplate::Blank => tr!("discord.space-sheet.own"),
        SpaceTemplate::Friends => tr!("discord.space-sheet.template-friends"),
        SpaceTemplate::StudyGroup => tr!("discord.space-sheet.template-study-group"),
        SpaceTemplate::SchoolClub => tr!("discord.space-sheet.template-school-club"),
        SpaceTemplate::LocalCommunity => tr!("discord.space-sheet.template-local-community"),
        SpaceTemplate::Creators => tr!("discord.space-sheet.template-creators"),
    }
}

fn symbol(template: SpaceTemplate) -> MaterialSymbol {
    match template {
        SpaceTemplate::Blank => MaterialSymbol::BRUSH,
        SpaceTemplate::Friends => MaterialSymbol::FAVORITE,
        SpaceTemplate::StudyGroup => MaterialSymbol::MENU_BOOK,
        SpaceTemplate::SchoolClub => MaterialSymbol::SCHOOL,
        SpaceTemplate::LocalCommunity => MaterialSymbol::PARK,
        SpaceTemplate::Creators => MaterialSymbol::PALETTE,
    }
}
