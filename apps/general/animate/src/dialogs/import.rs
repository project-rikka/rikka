use rikka_app::prelude::*;
use rikka_widgets::column_of;

use super::{STAGE, labelled};
use crate::animate::{Animate, Choosing};
use crate::arrival::{ClipSummary, Usage};

const WIDTH: f32 = 420.0;

pub(super) fn sheet(choosing: &Choosing, subject: Option<&str>, ready: bool) -> Element {
    let choice = choosing.choice();
    let clips = choice.clips.len() as u64;
    let summary = match choice.figure {
        true => tr!("animate.import.summary_figure", clips = clips),
        false => tr!("animate.import.summary_motion", clips = clips),
    };
    let mut rows: Vec<Element> = Vec::new();
    if choice.figure && !choice.clips.is_empty() {
        rows.push(labelled(tr!("animate.import.use"), usage_picker(choosing, subject)).build());
    }
    if !choice.clips.is_empty() {
        rows.push(labelled(tr!("animate.import.motion"), clip_picker(choosing)).build());
    }
    let accept = match choosing.usage() {
        Usage::Figure { .. } => tr!("animate.import.add_figure"),
        Usage::Motion { .. } => tr!("animate.import.apply"),
    };
    dialog(
        tr!("animate.import.title", file = choice.file().as_str()),
        column_of(rows).gap(Space::ROOMY).fill_width(),
    )
    .summary(summary)
    .cancel(tr!("animate.import.skip"), |animate: &mut Animate| {
        animate.skip_choice();
    })
    .accept(accept, |animate: &mut Animate| animate.settle_choice())
    .ready(ready)
    .width(WIDTH)
    .keyed(("animate-import-sheet", choice.file()))
    .centered_on(STAGE)
    .build()
}

fn usage_picker(choosing: &Choosing, subject: Option<&str>) -> impl View {
    let mut options = vec![tr!("animate.import.as_figure")];
    if let Some(subject) = subject {
        options.push(tr!("animate.import.as_motion", figure = subject));
    }
    let chosen = match choosing.usage() {
        Usage::Figure { .. } => 0,
        Usage::Motion { .. } => 1,
    };
    combo(Some(chosen))
        .options(options)
        .on_select(|animate: &mut Animate, chosen: usize| animate.choose_usage(chosen == 0))
        .grow(1.0)
}

fn clip_picker(choosing: &Choosing) -> impl View {
    let named = |index: usize, clip: &ClipSummary| {
        let name = match clip.name.trim().is_empty() {
            true => tr!("animate.import.unnamed", number = index as u64 + 1).to_string(),
            false => clip.name.clone(),
        };
        tr!(
            "animate.import.clip",
            name = name.as_str(),
            seconds = format!("{:.1}", clip.seconds).as_str()
        )
    };
    let clips = choosing
        .choice()
        .clips
        .iter()
        .enumerate()
        .map(|(index, clip)| named(index, clip));
    let (options, chosen): (Vec<Label>, usize) = match choosing.usage() {
        Usage::Figure { clip } => (
            std::iter::once(tr!("animate.import.no_motion"))
                .chain(clips)
                .collect(),
            clip.map_or(0, |clip| clip + 1),
        ),
        Usage::Motion { clip } => (clips.collect(), clip),
    };
    let still = matches!(choosing.usage(), Usage::Figure { .. });
    combo(Some(chosen))
        .options(options)
        .on_select(move |animate: &mut Animate, chosen: usize| {
            animate.choose_clip(match still {
                true => chosen.checked_sub(1),
                false => Some(chosen),
            });
        })
        .grow(1.0)
}
