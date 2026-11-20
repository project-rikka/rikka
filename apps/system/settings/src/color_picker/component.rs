use rikka_app::prelude::*;
use rikka_color_formats::{Formatter, Parameter, Rgb, Template};
use rikka_settings::session::color_picker::{
    self as offered, ACTIVATION, CLOSE, COPIED_FORMAT, DOMAIN, EDITOR, FORMATS, Format,
    FormatError, HEX, MIDDLE_CLICK, PICK, PICK_AND_CLOSE, PICK_THEN_EDITOR, PRIMARY_CLICK,
    SECONDARY_CLICK, SHOW_NAME,
};
use rikka_widgets::{column, row};

use crate::color_picker::editing::Editing;
use crate::controls::{Choices, choice, flag};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::picker;
use crate::session::Session;

const ACTIVATIONS: Choices = &[
    (PICK, "settings.color-picker.activations.pick"),
    (EDITOR, "settings.color-picker.activations.editor"),
];
const CLICKS: Choices = &[
    (
        PICK_THEN_EDITOR,
        "settings.color-picker.clicks.pick-then-editor",
    ),
    (
        PICK_AND_CLOSE,
        "settings.color-picker.clicks.pick-and-close",
    ),
    (CLOSE, "settings.color-picker.clicks.close"),
];

const NAME: f32 = 220.0;
const FORMAT: f32 = 340.0;
const CODE: f32 = 52.0;

pub(crate) struct ColorPicker {
    domain: Domain,
    editing: Option<Editing>,
}

impl Page for ColorPicker {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
    }

    fn closed(&mut self) {
        self.editing = None;
    }
}

domain_page!(ColorPicker);

impl ColorPicker {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            editing: None,
        }
    }

    fn formats(&self) -> Vec<Format> {
        self.domain
            .value(FORMATS)
            .map(|value| offered::formats(&value))
            .unwrap_or_default()
    }

    fn copied(&self) -> String {
        self.domain
            .non_empty_string(COPIED_FORMAT)
            .unwrap_or_else(|| HEX.to_owned())
    }

    fn write(&mut self, formats: &[Format]) {
        self.domain
            .set_value(FORMATS, offered::formats_value(formats));
    }

    fn copy_as(&mut self, index: usize) {
        if let Some(format) = self.formats().get(index) {
            self.domain.set(COPIED_FORMAT, format.name.as_str());
        }
    }

    fn show(&mut self, at: usize, shown: bool) {
        let mut formats = self.formats();
        if let Some(format) = formats.get_mut(at) {
            format.shown = shown;
            self.write(&formats);
        }
    }

    fn shift(&mut self, at: usize, later: bool) {
        let mut formats = self.formats();
        let to = match later {
            true => at + 1,
            false => at.wrapping_sub(1),
        };
        if at < formats.len() && to < formats.len() {
            formats.swap(at, to);
            self.write(&formats);
        }
    }

    fn delete(&mut self, at: usize) {
        let mut formats = self.formats();
        if at >= formats.len() {
            return;
        }
        let removed = formats.remove(at);
        self.editing = None;
        self.write(&formats);
        if removed.name == self.copied() {
            let next = formats.first().map_or(HEX, |format| format.name.as_str());
            self.domain.set(COPIED_FORMAT, next);
        }
    }

    fn add(&mut self) {
        self.editing = Some(Editing::adding());
    }

    fn edit(&mut self, at: usize) {
        if let Some(format) = self.formats().get(at) {
            self.editing = Some(Editing::changing(at, format));
        }
    }

    fn typed(&mut self) {
        if let Some(editing) = &mut self.editing {
            editing.error = None;
        }
    }

    fn explain(&mut self) {
        if let Some(editing) = &mut self.editing {
            editing.explained = !editing.explained;
        }
    }

    fn cancel(&mut self) {
        self.editing = None;
    }

    fn save(&mut self) {
        let formats = self.formats();
        let Some(editing) = &mut self.editing else {
            return;
        };
        let place = editing.place(&formats);
        let Some(committed) = editing.settled(&formats) else {
            return;
        };
        let before = formats.get(place).map(|format| format.name.clone());
        let name = committed[place].name.clone();
        self.editing = None;
        self.write(&committed);
        if before.is_some_and(|before| before != name && before == self.copied()) {
            self.domain.set(COPIED_FORMAT, name);
        }
    }

    fn restore(&mut self) {
        self.editing = None;
        self.domain.reset(FORMATS);
        if !offered::built_in_formats()
            .iter()
            .any(|format| format.name == self.copied())
        {
            self.domain.reset(COPIED_FORMAT);
        }
    }
}

pub(crate) fn view(page: &ColorPicker) -> impl View {
    let formats = page.formats();
    column! {
        domain_notice(
            &page.domain,
            "settings.color-picker.unavailable",
            "settings.color-picker.refused",
        );
        section(tr!("settings.color-picker.theme_menu_open"), activation(page));
        section(tr!("settings.color-picker.copying"), copying(page, &formats));
        section(tr!("settings.color-picker.formats"), listed(page, &formats));
    }
    .gap(Space::ROOMY)
}

fn activation(page: &ColorPicker) -> impl View {
    column! {
        choice(page, tr!("settings.color-picker.activation"), ACTIVATION, ACTIVATIONS);
        choice(page, tr!("settings.color-picker.primary-click"), PRIMARY_CLICK, CLICKS);
        choice(page, tr!("settings.color-picker.middle-click"), MIDDLE_CLICK, CLICKS);
        choice(page, tr!("settings.color-picker.secondary-click"), SECONDARY_CLICK, CLICKS);
    }
    .gap(Space::SNUG)
}

fn copying(page: &ColorPicker, formats: &[Format]) -> impl View {
    let copied = page.copied();
    let at = formats.iter().position(|format| format.name == copied);
    column! {
        field(
            tr!("settings.color-picker.copied"),
            picker(
                at,
                formats.iter().map(|format| Label::from(format.name.as_str())),
                |page: &mut ColorPicker, index: usize| page.copy_as(index),
            ),
        );
        flag(page, tr!("settings.color-picker.show-name"), SHOW_NAME);
    }
    .gap(Space::SNUG)
}

fn listed(page: &ColorPicker, formats: &[Format]) -> impl View {
    let editing = page.editing.as_ref();
    let count = formats.len();
    column! {
        for (at, format) in formats.iter().enumerate() {
            match editing.filter(|editing| editing.at == Some(at)) {
                Some(editing) => editor(editing).any(),
                None => listed_row(at, format, count).any(),
            }
        }
        if let Some(editing) = editing.filter(|editing| editing.at.is_none()) {
            editor(editing);
        }
        row! {
            button(
                row! {
                    icon(MaterialSymbol::ADD);
                    text(tr!("settings.color-picker.add"))
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .secondary()
            .small()
            .enabled(editing.is_none())
            .on_press(ColorPicker::add);
            button(text(tr!("settings.color-picker.restore")))
                .ghost()
                .small()
                .on_press(ColorPicker::restore);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
    }
    .gap(Space::SNUG)
}

fn listed_row(at: usize, format: &Format, count: usize) -> impl View {
    let example = Template::parse(&format.format).render(Rgb::MOCCASIN);
    row! {
        column! {
            text(format.name.as_str()).label().no_wrap();
            text(example).caption().muted().family(Family::Monospace).no_wrap();
        }
        .gap(Space::from(2.0))
        .grow(1.0)
        .shrink(1.0)
        .min_width(Length::px(0.0));
        switch(format.shown)
            .on_toggle(move |page: &mut ColorPicker, on: bool| page.show(at, on));
        button(icon(MaterialSymbol::ARROW_UPWARD))
            .ghost()
            .icon_only()
            .enabled(at > 0)
            .on_press(move |page: &mut ColorPicker| page.shift(at, false));
        button(icon(MaterialSymbol::ARROW_DOWNWARD))
            .ghost()
            .icon_only()
            .enabled(at + 1 < count)
            .on_press(move |page: &mut ColorPicker| page.shift(at, true));
        button(icon(MaterialSymbol::EDIT))
            .ghost()
            .icon_only()
            .on_press(move |page: &mut ColorPicker| page.edit(at));
        button(icon(MaterialSymbol::DELETE))
            .ghost()
            .icon_only()
            .on_press(move |page: &mut ColorPicker| page.delete(at));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn editor(editing: &Editing) -> impl View {
    let title = match editing.at {
        Some(_) => tr!("settings.color-picker.editing"),
        None => tr!("settings.color-picker.adding"),
    };
    let error = editing.error.as_ref().map(|error| match error {
        FormatError::MissingName => tr!("settings.color-picker.nameless"),
        FormatError::DuplicateName(name) => {
            tr!("settings.color-picker.taken", name = name.as_str())
        }
    });
    let explaining = match editing.explained {
        true => tr!("settings.color-picker.hide-parameters"),
        false => tr!("settings.color-picker.show-parameters"),
    };
    let example = tr!(
        "settings.color-picker.example",
        example = editing.example().as_str()
    );
    container(
        column! {
            text(title).label();
            field(
                tr!("settings.color-picker.name"),
                text_field(&editing.name)
                    .on_change(ColorPicker::typed)
                    .on_submit(ColorPicker::save)
                    .width(NAME),
            );
            field(
                tr!("settings.color-picker.format"),
                text_field(&editing.format)
                    .on_change(ColorPicker::typed)
                    .on_submit(ColorPicker::save)
                    .width(FORMAT),
            );
            text(example).caption().muted();
            error.map(|error| text(error).caption().danger());
            button(text(explaining))
                .ghost()
                .small()
                .on_press(ColorPicker::explain)
                .align_self(Align::Start);
            if editing.explained {
                parameters();
            }
            row! {
                spacer();
                button(text(tr!("settings.color-picker.cancel")))
                    .secondary()
                    .small()
                    .on_press(ColorPicker::cancel);
                button(text(tr!("settings.color-picker.save")))
                    .small()
                    .on_press(ColorPicker::save);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
    .border(1.0, palette().accent)
    .fill_width()
}

fn parameters() -> impl View {
    let explained = |code: String, label: Label| {
        row! {
            text(code).caption().family(Family::Monospace).width(CODE);
            text(label).caption().muted()
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
    };
    column! {
        text(tr!("settings.color-picker.parameters")).caption().medium();
        for parameter in Parameter::ALL {
            explained(format!("%{}", parameter.code()), parameter.label());
        }
        text(tr!("settings.color-picker.formatters")).caption().medium();
        for formatter in Formatter::CHANNELS {
            explained(formatter.letter().to_string(), formatter.label());
        }
        text(tr!("settings.color-picker.lab-formatters")).caption().medium();
        for formatter in Formatter::LAB {
            explained(formatter.letter().to_string(), formatter.label());
        }
        text(tr!("settings.color-picker.formatter-example")).caption().muted();
    }
    .gap(Space::TIGHT)
    .fill_width()
}

impl std::fmt::Debug for ColorPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorPicker")
            .field("editing", &self.editing)
            .finish_non_exhaustive()
    }
}
