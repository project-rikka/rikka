use rikka_thread::OutputLog;

use crate::card::{CARD_INSET, card, chevron};
use crate::mono::{CODE_FONT_SIZE, LINE_HEIGHT_FACTOR, mono};
use crate::prelude::*;
use crate::printed::PrintedDocuments;
use crate::transcript::Transcript;
use rikka_widgets::{column, row};

const MAX_OUTPUT_HEIGHT: f32 = 300.0;
const CHEVRON_SIZE: f32 = 16.0;
const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;

pub(crate) struct CommandCall<'a> {
    pub(crate) call: &'a str,
    pub(crate) command: &'a str,
    pub(crate) output: Option<&'a str>,
    pub(crate) log: Option<&'a OutputLog>,
    pub(crate) failed: bool,
    pub(crate) open: bool,
}

pub(crate) fn view(command: CommandCall<'_>, documents: &PrintedDocuments) -> impl View {
    let inline_output = command
        .output
        .map(str::trim_end)
        .filter(|output| !output.is_empty());
    let log_output = command
        .log
        .filter(|_| command.open)
        .map(|log| fetch_log(command.call, log));
    let shown_output = log_output.as_ref().and_then(fetched_text).or(inline_output);

    let output = shown_output.filter(|_| command.open).map(|output| {
        column! {
            rule();
            output_view(command.call, output, documents);
        }
        .fill_width()
    });

    card(
        column! {
            command_view(&command, shown_output.is_some());
            collapsible(output).reveals_on_open().fill_width();
        },
        Inset::NONE,
        palette().outline,
    )
    .clip_content()
}

fn fetch_log(call: &str, log: &OutputLog) -> Fetch<String> {
    let reader = log.clone();
    fetch_blocking((call, log.at.as_str()), move || reader.read(MAX_LOG_BYTES))
}

fn fetched_text(fetch: &Fetch<String>) -> Option<&str> {
    match fetch {
        Fetch::Ready(output) => Some(output),
        Fetch::Failed(error) => Some(error),
        Fetch::Loading => None,
    }
}

fn command_view(command: &CommandCall<'_>, has_output: bool) -> impl View {
    let palette = palette();
    let open = command.open;
    let line = row! {
        mono("$", CODE_FONT_SIZE)
            .color(match command.failed {
                true => palette.danger,
                false => palette.accent,
            })
            .no_wrap();
        mono(command.command, CODE_FONT_SIZE).color(palette.text).grow(1.0);
        if has_output {
            icon(chevron(open))
                .extent(Length::px(CHEVRON_SIZE))
                .color(palette.text_muted);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width();

    let heading = container(line)
        .padding(Inset::xy(CARD_INSET, 8.0))
        .fill_width();
    match has_output {
        true => {
            let call = command.call.to_owned();
            heading
                .on_press(move |transcript: &mut Transcript| transcript.fold_output(call.clone()))
                .focusless()
        }
        false => heading,
    }
}

fn output_view(call: &str, output: &str, documents: &PrintedDocuments) -> impl View {
    container(
        text_editor(&documents.document(call, output))
            .read_only(true)
            .gutter(false)
            .fits_text(0.0..=MAX_OUTPUT_HEIGHT)
            .font_size(CODE_FONT_SIZE)
            .line_height((CODE_FONT_SIZE * LINE_HEIGHT_FACTOR).round())
            .family(Family::parse(font::JETBRAINS_MONO))
            .color(palette().text_muted)
            .fill_width(),
    )
    .padding(Inset::xy(CARD_INSET, 8.0))
    .fill_width()
}
