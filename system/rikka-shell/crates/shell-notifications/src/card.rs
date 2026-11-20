use rikka_notifications::{Body, Notification, Progress, Style, Urgency};
use rikka_theme::{Corner, Palette};
use rikka_types::Color;
use rikka_ui::{
    Align, Element, FontStyle, Handler, IconSource, Inset, IntoHandler, Length, MaterialSymbol,
    Space, Styled, TextSpan, TextStyled, View, font, palette, text, tr,
};
use rikka_widgets::{
    ButtonVariant, ControlSize, ImageFit, button, column, container, icon, image, progress,
    progress_indeterminate, rich_text, row, spacer,
};
use shell_kit::clip;

use crate::look::{Button, Look};

const BORDER: f32 = 1.0;

const CLOSE_ICON: f32 = 14.0;

const ACTION_ICON: f32 = 16.0;

#[must_use]
pub fn card<'a, Marker>(
    notification: &'a Notification,
    look: &'a Look,
    on_press: impl IntoHandler<(), (), Marker>,
) -> Card<'a> {
    Card {
        notification,
        look,
        at: None,
        icon: 0.0,
        radius: 0.0,
        inset: 0.0,
        glass: false,
        on_press: on_press.into_handler(),
        on_close: None,
        on_act: None,
        on_follow: None,
    }
}

#[derive(Debug)]
pub struct Card<'a> {
    notification: &'a Notification,
    look: &'a Look,
    at: Option<String>,
    icon: f32,
    radius: f32,
    inset: f32,
    glass: bool,
    on_press: Handler,
    on_close: Option<Handler>,
    on_act: Option<Handler<(String,)>>,
    on_follow: Option<Handler<(usize,)>>,
}

impl Card<'_> {
    #[must_use]
    pub fn sized(mut self, icon: f32, radius: f32, inset: f32) -> Self {
        self.icon = icon;
        self.radius = radius;
        self.inset = inset;
        self
    }

    #[must_use]
    pub(crate) fn glass(mut self) -> Self {
        self.glass = true;
        self
    }

    #[must_use]
    pub fn at(mut self, at: String) -> Self {
        self.at = Some(at);
        self
    }

    #[must_use]
    pub fn closed<Marker>(mut self, on_close: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_close = Some(on_close.into_handler());
        self
    }

    #[must_use]
    pub fn acted<Marker>(mut self, on_act: impl IntoHandler<(String,), (), Marker>) -> Self {
        self.on_act = Some(on_act.into_handler());
        self
    }

    #[must_use]
    pub fn followed<Marker>(mut self, on_follow: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_follow = Some(on_follow.into_handler());
        self
    }

    #[must_use]
    pub fn build(self) -> Element {
        let Card {
            notification,
            look,
            at,
            icon,
            radius,
            inset,
            glass,
            on_press,
            on_close,
            on_act,
            on_follow,
        } = self;
        let palette = palette();

        let posted_at = at.unwrap_or_else(|| look.at.clone());
        let lines = column! {
            top(notification, &posted_at, on_close);
            if !notification.summary.is_empty() {
                text(notification.summary.as_str())
                    .label()
                    .color(palette.text);
            }
            if !notification.body.is_empty() {
                prose(&notification.body, on_follow);
            }
            if let Some(progress) = notification.progress {
                meter(progress);
            }
            if let Some(on_act) = on_act {
                actions(&look.buttons, &on_act);
            }
        }
        .gap(Space::HAIR);

        let card = container(
            row! {
                badge(look, icon);
                clip(lines)
            }
            .gap(Space::NORMAL)
            .align_items(Align::Start)
            .fill_width(),
        )
        .padding(inset)
        .radius(radius)
        .background(palette.surface)
        .border(BORDER, edge(notification.urgency, palette))
        .fill_width()
        .resizing()
        .on_press(on_press);

        match glass {
            true => card.glass().build(),
            false => card.build(),
        }
    }
}

fn top(notification: &Notification, posted_at: &str, on_close: Option<Handler>) -> impl View {
    let palette = palette();
    let app_name = notification.app_name.as_str();
    row! {
        match app_name.is_empty() {
            true => spacer().grow(1.0).build(),
            false => clip(
                text(app_name)
                    .caption()
                    .color(palette.text)
                    .medium()
                    .no_wrap(),
            )
            .build(),
        }
        text(posted_at).caption().muted().no_wrap();
        if let Some(on_close) = on_close {
            close(on_close, palette);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn close(on_close: Handler, palette: Palette) -> impl View {
    container(
        icon(MaterialSymbol::CLOSE)
            .extent(Length::px(CLOSE_ICON))
            .color(palette.text_muted),
    )
    .padding(Space::TIGHT)
    .radius(Corner::Item)
    .background(palette.surface_variant)
    .focusless()
    .on_press(on_close)
}

fn prose(body: &Body, on_follow: Option<Handler<(usize,)>>) -> impl View {
    let mut spans: Vec<TextSpan> = body
        .emphasis
        .iter()
        .map(|emphasis| {
            let span = TextSpan::new(emphasis.range.clone());
            match emphasis.style {
                Style::Bold => span.weight(font::BOLD),
                Style::Italic => span.style(FontStyle::Italic),
                Style::Underline => span.underline(true),
            }
        })
        .collect();
    let mut prose = rich_text(body.text.as_str()).caption().muted();
    if let Some(on_follow) = on_follow {
        spans.extend(body.links.iter().map(|link| {
            TextSpan::new(link.range.clone())
                .color(palette().accent)
                .underline(true)
        }));
        prose = prose
            .links(
                body.links
                    .iter()
                    .map(|link| link.range.clone())
                    .collect::<Vec<_>>(),
            )
            .on_link(on_follow);
    }
    prose.spans(spans)
}

fn meter(shown: Progress) -> impl View {
    let bar = match shown.fraction() {
        Some(fraction) => progress(fraction),
        None => progress_indeterminate(),
    };
    container(bar.control_size(ControlSize::Small).fill_width())
        .padding(Inset::edges(6.0, 0.0, 2.0, 0.0))
        .fill_width()
}

fn actions(action_buttons: &[Button], on_act: &Handler<(String,)>) -> Option<impl View> {
    if action_buttons.is_empty() {
        return None;
    }
    Some(
        row! {
            for drawn in action_buttons {
                pressable(drawn, on_act);
            }
        }
        .gap(Space::SNUG)
        .fill_width()
        .padding(Inset::edges(6.0, 0.0, 0.0, 0.0)),
    )
}

fn pressable(drawn: &Button, on_act: &Handler<(String,)>) -> impl View {
    let face = match &drawn.icon {
        Some(source) => icon(source.clone()).extent(Length::px(ACTION_ICON)).build(),
        None => text(drawn.label.as_str()).caption().no_wrap().build(),
    };
    let key = drawn.key.clone();
    button(face)
        .variant(ButtonVariant::Secondary)
        .control_size(ControlSize::Small)
        .on_press(on_act.clone().map(move |()| (key.clone(),)))
}

fn badge(look: &Look, size: f32) -> Element {
    match &look.icon {
        IconSource::Glyph(_) | IconSource::Mask(_) => icon(look.icon.clone())
            .extent(Length::px(size))
            .color(palette().accent)
            .build(),
        IconSource::Picture(picture) => image(picture.clone())
            .fit(ImageFit::Contain)
            .radius(Corner::Item)
            .width(size)
            .height(size)
            .build(),
    }
}

#[must_use]
pub fn posted_at(look: &Look, today: Option<jiff::civil::Date>) -> String {
    match (look.on, today) {
        (Some(on), Some(today)) if on != today => tr!(
            "shell.centre.said-on",
            month = on.month(),
            day = on.day(),
            time = look.at.as_str()
        )
        .to_string(),
        _ => look.at.clone(),
    }
}

#[must_use]
fn edge(urgency: Urgency, palette: Palette) -> Color {
    match urgency {
        Urgency::Critical => palette.danger,
        Urgency::Low | Urgency::Normal => palette.outline,
    }
}
