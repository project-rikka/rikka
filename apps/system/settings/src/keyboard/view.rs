use rikka_app::prelude::*;
use rikka_ime::InputSource;
use rikka_widgets::{column, row};

use crate::domain::DomainPage;
use crate::keyboard::component::{self, Keyboard};
use crate::keyboard::pane::Pane;
use crate::keyboard::{catalogue, console, options, shortcuts};
use crate::notice::{domain_notice, notice};
use crate::pending::Pending;
use crate::picker::value_picker;
use crate::settings::Settings;

const OFFERS: f32 = 320.0;

pub(crate) fn view(keyboard: &Keyboard, pending: &Pending) -> impl View {
    column! {
        panes(keyboard.pane());
        page_stack(
            keyboard.pane().index(),
            match keyboard.pane() {
                Pane::Input => input(keyboard, pending).build(),
                Pane::Shortcuts => shortcuts::view(keyboard.shortcuts()).build(),
            },
        )
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn panes(pane: Pane) -> impl View {
    segmented(pane.index())
        .segments(Pane::ALL.map(Pane::title))
        .on_select(|keyboard: &mut Keyboard, index: usize| keyboard.show(Pane::from_index(index)))
        .fill_width()
}

fn input(keyboard: &Keyboard, pending: &Pending) -> impl View {
    column! {
        domain_notice(
            keyboard.domain(),
            "settings.keyboard.unavailable",
            "settings.keyboard.refused",
        );
        keyboard.error().map(|reason| {
            notice(
                MaterialSymbol::ERROR,
                palette().danger,
                tr!("settings.keyboard.method-refused", reason = reason),
            )
        });
        keyboard
            .unreadable()
            .map(|reason| text(reason.to_owned()).label().muted());
        if keyboard.adding().is_some() {
            adding(keyboard, pending);
        } else {
            sources(keyboard);
            if pending.unavailable().is_none() {
                console::card(keyboard.layouts(), keyboard.keymaps(), pending);
            }
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn sources(keyboard: &Keyboard) -> Element {
    let sources = keyboard.sources();
    let last = sources.len().saturating_sub(1);
    keyboard.sources_section(section(
        tr!("settings.keyboard.sources"),
        column! {
            for (index, source) in sources.iter().enumerate() {
                row_for(keyboard, index, source, last);
            }
            spacer().height(4.0);
            footer(keyboard);
        }
        .gap(Space::TIGHT)
        .fill_width(),
    ))
}

fn footer(keyboard: &Keyboard) -> impl View {
    row! {
        button(
            row! {
                icon(MaterialSymbol::ADD).extent(Length::px(16.0));
                text(tr!("settings.keyboard.add")).label();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .secondary()
        .small()
        .on_press(|keyboard: &mut Keyboard| keyboard.open_adding());
        spacer().grow(1.0);
        match keyboard.ime_unavailable() {
            Some(_) => text(tr!("settings.keyboard.method-none")).caption().muted(),
            None => button(tr!("settings.keyboard.configure"))
                .ghost()
                .small()
                .on_press(component::configure),
        }
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
}

fn row_for(keyboard: &Keyboard, index: usize, source: &InputSource, last: usize) -> impl View {
    container(
        row! {
            text(keyboard.describe(source)).label().no_wrap();
            spacer();
            variants(keyboard, source, index);
            step(MaterialSymbol::ARROW_UPWARD, index, -1, index > 0);
            step(MaterialSymbol::ARROW_DOWNWARD, index, 1, index < last);
            button(icon(MaterialSymbol::CLOSE))
                .ghost()
                .small()
                .icon_only()
                .enabled(last != 0)
                .on_press(move |keyboard: &mut Keyboard| keyboard.remove(index));
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .padding(Space::SNUG)
}

fn step(symbol: MaterialSymbol, index: usize, step: isize, enabled: bool) -> impl View {
    button(icon(symbol))
        .ghost()
        .small()
        .icon_only()
        .enabled(enabled)
        .on_press(move |keyboard: &mut Keyboard| keyboard.shift(index, step))
}

fn variants(keyboard: &Keyboard, source: &InputSource, index: usize) -> Option<impl View> {
    let offered = options::variants(keyboard.layouts().layout(&source.layout));
    if offered.is_empty() {
        return None;
    }
    Some(value_picker(
        tr!("settings.keyboard.plain-layout"),
        offered,
        Some(source.variant.as_str()).filter(|variant| !variant.is_empty()),
        move |keyboard: &mut Keyboard, variant: Option<String>| {
            keyboard.set_variant(index, variant);
        },
    ))
}

fn adding(keyboard: &Keyboard, pending: &Pending) -> impl View {
    let offers = catalogue::offers(keyboard, pending);
    let query = keyboard.adding().expect("the sheet holds what was typed");

    card(
        column! {
            row! {
                button(icon(MaterialSymbol::ARROW_BACK))
                    .ghost()
                    .icon_only()
                    .on_press(|keyboard: &mut Keyboard| keyboard.close_adding());
                text(tr!("settings.keyboard.add-title")).body().semibold();
                spacer().grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            text_field(query)
                .placeholder(tr!("settings.keyboard.search"))
                .on_change(|| {})
                .fill_width();
            if offers.is_empty() {
                text(tr!("settings.keyboard.nothing")).label().muted();
            } else {
                list(catalogue::rows(keyboard, &offers))
                    .selected(None)
                    .control_size(ControlSize::Small)
                    .on_select(|settings: &mut Settings, index: usize| {
                        settings.choose_offer(index);
                    })
                    .height(OFFERS)
                    .fill_width();
            }
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}
