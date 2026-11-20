use rikka_theme::Corner;
use rikka_types::{Color, Key, Modifiers};
use rikka_ui::{
    Align, Element, ImageSource, Inset, IntoHandler, Label, Length, MaterialSymbol, Space, Styled,
    TextColor, TextStyled, View, palette, text, tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, ImageFit, button, column, container, icon, image, row, scrollable,
    spacer, text_field,
};
use shell_kit::{clip, finder, nothing, popover};

use crate::entry::{Content, Entry, Picture};
use crate::picker::Picker;

const WIDTH: f32 = 380.0;
const ROWS_HEIGHT: f32 = 360.0;

const BORDER: f32 = 1.0;
const INSET: f32 = 10.0;

const PICTURE_HEIGHT: f32 = 104.0;
const ACTION_ICON: f32 = 16.0;
const ACTION_PADDING: f32 = 4.0;

const ROWS: &str = "clipboard.rows";

pub(crate) fn view(picker: &Picker) -> impl View {
    let selected = picker.selected();

    popover(contents(picker), |picker: &mut Picker| picker.close())
        .width(WIDTH)
        .beside(picker.aim())
        .on_key(move |picker: &mut Picker, key: Key, modifiers: Modifiers| {
            on_key(picker, key, modifiers, selected)
        })
}

fn on_key(picker: &mut Picker, key: Key, modifiers: Modifiers, selected: usize) -> bool {
    let control = modifiers.control;
    match key.to_lowercase() {
        Key::ArrowDown => picker.walk(1),
        Key::ArrowUp => picker.walk(-1),
        Key::PageDown => picker.walk(PAGE),
        Key::PageUp => picker.walk(-PAGE),
        Key::Escape => picker.escape(),
        Key::Enter if control => picker.copy(selected),
        Key::Char('d') if control && modifiers.shift => picker.clear(),
        Key::Char('d') if control => picker.forget(selected),
        Key::Char('p') if control => picker.pin(selected),
        _ => return false,
    }
    true
}

const PAGE: i32 = 8;

fn contents(picker: &Picker) -> impl View {
    let palette = palette();
    let contents = column! {
        heading(picker);
        search(picker);
        if let Some(trouble) = picker.trouble() {
            container(text(trouble).caption().color(TextColor::Warning))
                .padding(Inset::xy(INSET, ACTION_PADDING));
        }
        rows(picker).keyed(ROWS)
    }
    .fill_width()
    .padding(Space::TIGHT);

    container(contents)
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .resizing()
}

fn heading(picker: &Picker) -> impl View {
    row! {
        text(tr!("shell.clipboard.title")).caption().muted().medium();
        spacer().grow(1.0);
        if !picker.is_empty() {
            button(text(clearing(picker)).caption())
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .on_press(Picker::clear);
        }
    }
    .align_items(Align::Center)
    .padding(Inset::xy(INSET, 4.0))
}

fn clearing(picker: &Picker) -> Label {
    match picker.has_pinned() {
        true => tr!("shell.clipboard.clear-unpinned"),
        false => tr!("shell.clipboard.clear"),
    }
}

fn search(picker: &Picker) -> impl View {
    let selected = picker.selected();
    finder(
        text_field(picker.field())
            .placeholder(tr!("shell.clipboard.search"))
            .on_change(Picker::typed)
            .on_submit(move |picker: &mut Picker| picker.paste(selected)),
    )
}

fn rows(picker: &Picker) -> Element {
    if picker.count() == 0 {
        return match picker.is_empty() {
            true => nothing(MaterialSymbol::INBOX, tr!("shell.clipboard.empty")).build(),
            false => nothing(MaterialSymbol::SEARCH_OFF, tr!("shell.clipboard.no-match")).build(),
        };
    }
    let list = column! {
        for (at, entry) in picker.shown().enumerate() {
            clipping(entry, at, at == picker.selected()).build().keyed(entry.id);
        }
    }
    .fill_width()
    .gap(Space::TIGHT);
    scrollable(list).max_height(ROWS_HEIGHT).build()
}

fn clipping(entry: &Entry, at: usize, selected: bool) -> impl View {
    let palette = palette();
    let (pin, pinning) = match entry.pinned {
        true => (MaterialSymbol::KEEP, palette.accent),
        false => (MaterialSymbol::KEEP_OFF, palette.text_muted),
    };
    let buttons = column! {
        action(MaterialSymbol::CLOSE, palette.text_muted, move |picker: &mut Picker| {
            picker.forget(at)
        });
        spacer().grow(1.0);
        action(pin, pinning, move |picker: &mut Picker| picker.pin(at))
    }
    .align_items(Align::End);

    let line = row! {
        clip(body(entry));
        buttons
    }
    .gap(Space::SNUG)
    .fill_width();

    let card = container(line)
        .padding(Space::SNUG)
        .radius(Corner::Panel)
        .background(palette.surface_variant)
        .on_press(move |picker: &mut Picker| picker.paste(at));
    match selected {
        true => card.border(BORDER, palette.accent),
        false => card,
    }
}

fn body(entry: &Entry) -> Element {
    let palette = palette();
    let second = match &entry.content {
        Content::Image(picture) => return picture_of(picture).build(),
        Content::Text(_) => entry.preview.rest.as_deref().map(Label::from),
        Content::Files(_) => Some(tr!("shell.clipboard.files")),
    };

    column! {
        text(entry.preview.title.as_str()).label().color(palette.text).no_wrap();
        if let Some(second) = second {
            text(second).caption().muted().no_wrap();
        }
    }
    .gap(Space::HAIR)
    .build()
}

fn picture_of(picture: &Picture) -> impl View {
    let across = PICTURE_HEIGHT * picture.thumbnail_width.max(1) as f32
        / picture.thumbnail_height.max(1) as f32;
    image(ImageSource::Rgba {
        width: picture.thumbnail_width,
        height: picture.thumbnail_height,
        pixels: picture.thumbnail.clone(),
    })
    .fit(ImageFit::Contain)
    .radius(Corner::Item)
    .width(across)
    .max_width(Length::FILL)
    .height(PICTURE_HEIGHT)
}

fn action<Marker>(
    symbol: MaterialSymbol,
    color: Color,
    press: impl IntoHandler<(), (), Marker>,
) -> impl View {
    container(icon(symbol).extent(Length::px(ACTION_ICON)).color(color))
        .padding(Space::TIGHT)
        .radius(Corner::Item)
        .focusless()
        .on_press(press)
}
