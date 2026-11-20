use rikka_thread::{Job, Kind};

use crate::chat::Chat;
use crate::models::named;
use crate::prelude::*;
use rikka_widgets::{column, row};

const GLYPH: f32 = 15.0;
const CROSS: f32 = 13.0;
const WIDEST: f32 = 320.0;
const PITCH: f32 = 1.25;

pub(crate) fn view<C: Chat>(jobs: &[Job], watching: Option<u64>) -> Option<Flex> {
    let listed: Vec<&Job> = jobs
        .iter()
        .filter(|job| job.state().is_running() || watching == Some(job.id()))
        .collect();
    if listed.is_empty() && watching.is_none() {
        return None;
    }

    let thread = watching.map(|_| thread::<C>());
    let chips = listed
        .into_iter()
        .map(move |job| chip::<C>(job, watching).any());

    Some(
        row_of(thread.into_iter().chain(chips))
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .wrap_items()
            .fill_width(),
    )
}

fn thread<C: Chat>() -> Element {
    let back = frame(
        row! {
            icon(MaterialSymbol::FORUM)
                .extent(Length::px(GLYPH))
                .color(palette().accent);
            text(C::main_thread()).caption().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
        false,
    );
    match C::watch_main() {
        Some(watch) => back.on_press(watch).any(),
        None => back.any(),
    }
}

fn chip<C: Chat>(job: &Job, watching: Option<u64>) -> impl View {
    let palette = palette();
    let id = job.id();
    let agent = job.kind() == Kind::Agent;
    let expanded = watching == Some(id);
    let model = job.on().map(|on| named(on.model(), on.effort()));
    let leading = leading();

    let chip = frame(
        row! {
            icon(symbol(job.kind()))
                .extent(Length::px(GLYPH))
                .color(palette.accent);
            column! {
                text(flattened(job.label()))
                    .caption()
                    .no_wrap()
                    .line_height(leading);
                if let Some(model) = model {
                    text(model).caption().muted().no_wrap().line_height(leading);
                }
            }
            .gap(Space::NONE)
            .grow(1.0);
            if job.state().is_running() {
                button(
                    icon(MaterialSymbol::STOP_CIRCLE)
                        .extent(Length::px(CROSS))
                        .color(palette.text_muted),
                )
                .icon_only()
                .ghost()
                .small()
                .on_press(C::stop_job().map(move |()| (id,)));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
        expanded,
    );

    match C::watch_agent().filter(|_| agent) {
        Some(watch) => chip.on_press(watch.map(move |()| (id,))),
        None => chip,
    }
}

fn frame(content: impl View, expanded: bool) -> Container {
    let palette = palette();
    let (background, border) = match expanded {
        true => (palette.selection, palette.accent),
        false => (palette.surface, palette.outline),
    };

    container(content)
        .background(background)
        .radius(Corner::Control)
        .border(1.0, border)
        .padding(Inset::xy(10.0, 6.0))
        .max_width(WIDEST)
}

fn leading() -> f32 {
    (env().font_size * TextRole::Caption.scale() * PITCH).round()
}

fn symbol(kind: Kind) -> MaterialSymbol {
    match kind {
        Kind::Command => MaterialSymbol::TERMINAL,
        Kind::Agent => MaterialSymbol::HUB,
    }
}

fn flattened(text: &str) -> String {
    text.trim().replace('\n', " ")
}
