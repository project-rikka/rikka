use rikka_thread::{Choice, Question};

use crate::chat::Chat;
use crate::prelude::*;
use crate::question_form::{Pick, QuestionForm};
use rikka_widgets::{column, row};

const TALLEST: f32 = 280.0;

pub(crate) fn view<C: Chat>(asking: &QuestionForm) -> impl View {
    let question = asking.question();
    column! {
        heading(question, asking.at(), asking.count());
        scrollable(body::<C>(question, asking.pick()))
            .max_height(TALLEST)
            .fill_width();
        pick_summary::<C>(asking.pick());
        onwards::<C>(asking);
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn body<C: Chat>(question: &Question, pick: &Pick) -> impl View {
    column! {
        text(question.text().to_owned())
            .label()
            .color(palette().text)
            .fill_width();
        options::<C>(question, pick);
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn heading(question: &Question, at: usize, of: usize) -> impl View {
    row! {
        if !question.header().trim().is_empty() {
            question_chip(question.header());
        }
        spacer().grow(1.0);
        if of > 1 {
            text(format!("{} / {of}", at + 1)).caption().muted().no_wrap();
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .shrink(0.0)
    .fill_width()
}

pub fn question_chip(header: &str) -> impl View + use<> {
    let palette = palette();
    container(
        text(header.to_owned())
            .caption()
            .medium()
            .color(palette.on_selection)
            .no_wrap(),
    )
    .background(palette.selection)
    .radius(Corner::Item)
    .padding(Inset::xy(8.0, 3.0))
}

fn options<C: Chat>(question: &Question, pick: &Pick) -> impl View {
    let multi = question.is_multi_select();
    column_of(question.options().iter().enumerate().map(|(at, option)| {
        let is_selected = pick.is_chosen(at);
        let label = described(option);
        let choose = C::choose_option().map(move |()| (at,));
        match multi {
            true => checkbox(is_selected)
                .label(label)
                .control_size(ControlSize::Small)
                .on_toggle(choose.map(|(_,): (bool,)| ()))
                .any(),
            false => radio(is_selected)
                .label(label)
                .control_size(ControlSize::Small)
                .on_select(choose)
                .any(),
        }
    }))
    .gap(Space::SNUG)
    .fill_width()
}

fn described(option: &Choice) -> impl View {
    let palette = palette();
    column! {
        text(option.label().to_owned()).label().color(palette.text).fill_width();
        if !option.description().trim().is_empty() {
            text(option.description().to_owned()).caption().muted().fill_width();
        }
    }
    .gap(Space::HAIR)
    .fill_width()
}

fn pick_summary<C: Chat>(pick: &Pick) -> impl View {
    row! {
        icon(MaterialSymbol::EDIT)
            .extent(Length::px(16.0))
            .color(palette().text_muted);
        text_field(pick.wrote())
            .placeholder(C::write_answer())
            .control_size(ControlSize::Small)
            .on_change(C::write_in())
            .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .shrink(0.0)
    .fill_width()
}

fn onwards<C: Chat>(asking: &QuestionForm) -> impl View {
    row! {
        button(text(C::stop_asking()).caption())
            .ghost()
            .control_size(ControlSize::Small)
            .on_press(C::stop());
        spacer().grow(1.0);
        if !asking.is_first() {
            button(text(C::back()).caption())
                .secondary()
                .control_size(ControlSize::Small)
                .on_press(C::go_back());
        }
        button(
            text(match asking.is_last() {
                true => C::send_answers(),
                false => C::next(),
            })
            .caption(),
        )
        .control_size(ControlSize::Small)
        .enabled(!asking.is_last() || asking.is_answered())
        .on_press(C::go_ahead());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .shrink(0.0)
    .fill_width()
}
