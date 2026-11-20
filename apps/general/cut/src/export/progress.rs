use cut_export::{Job, Phase, Summary};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::wording;
use crate::cut::Cut;
use crate::view::STAGE;

const WIDTH: f32 = 520.0;
const PREVIEW_HEIGHT: f32 = 270.0;

pub(crate) fn sheet(cut: &Cut, job: &Job) -> Vec<Element> {
    let title = wording::phase(job.phase());
    let frame = match job.phase() {
        Phase::Failed(trouble) => dialog(
            title,
            text(trouble.to_string())
                .color(palette().danger)
                .fill_width(),
        )
        .accept(tr!("cut.export.close"), Cut::close_export),
        Phase::Done(summary) => dialog(title, done(cut, summary))
            .aside(
                button(text(tr!("cut.export.show")))
                    .ghost()
                    .on_press(Cut::show_export),
            )
            .action(button(text(tr!("cut.export.open"))).on_press(Cut::open_export_result))
            .accept(tr!("cut.export.close"), Cut::close_export),
        Phase::Preparing
        | Phase::Rendering
        | Phase::Finishing
        | Phase::Cancelling
        | Phase::Cancelled => dialog(title, running(cut, job)).action(
            button(text(tr!("cut.export.cancel")))
                .ghost()
                .enabled(job.can_cancel())
                .on_press(Cut::cancel_export),
        ),
    };
    frame
        .on_dismiss(|cut: &mut Cut| {
            cut.cancel_export();
            cut.close_export();
        })
        .width(WIDTH)
        .keyed(("cut-export-progress", 0))
        .centered_on(STAGE)
        .with_scrim()
}

fn screen(cut: &Cut) -> impl View {
    let picture = cut
        .export_preview()
        .map(|preview| image(preview).fit(ImageFit::Contain).fill().build());
    container(column_of(picture.into_iter().collect::<Vec<Element>>()).fill())
        .background(Color::BLACK)
        .radius(Corner::Item)
        .fill_width()
        .height(PREVIEW_HEIGHT)
}

fn running(cut: &Cut, job: &Job) -> impl View {
    let steady = matches!(job.phase(), Phase::Rendering);
    let bar = match steady {
        true => progress(job.fraction()),
        false => progress_indeterminate(),
    };
    let mut facts = vec![tr!(
        "cut.export.counted",
        done = job.done(),
        total = job.total()
    )];
    if let Some(pace) = job.pace().filter(|_| steady) {
        facts.push(tr!("cut.export.pace", pace = format!("{pace:.1}").as_str()));
    }
    if let Some(remaining) = job.remaining().filter(|_| steady) {
        facts.push(tr!(
            "cut.export.remaining",
            duration = rikka_format::format_duration(remaining).as_str()
        ));
    }
    column! {
        screen(cut);
        bar.fill_width();
        row! {
            text(wording::joined(&facts)).caption().grow(1.0);
            if let Some(backend) = job.backend() {
                text(wording::backend(backend)).caption().muted();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn done(cut: &Cut, summary: &Summary) -> impl View {
    let name = summary
        .path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut facts = Vec::new();
    if !summary.duration.is_zero() {
        facts.push(Label::from(rikka_format::format_duration(summary.duration)));
    }
    if summary.frames > 1 {
        facts.push(tr!("cut.export.frame_count", frames = summary.frames));
    }
    if let Some(backend) = summary.backend {
        facts.push(wording::backend(backend));
    }
    let mut warnings = Vec::new();
    if summary.offline > 0 {
        warnings.push(tr!("cut.export.went_dark", count = summary.offline));
    }
    if summary.undecodable > 0 {
        warnings.push(tr!("cut.export.unheard", count = summary.undecodable));
    }
    column! {
        screen(cut);
        column! {
            text(name).semibold().no_wrap();
            text(wording::joined(&facts)).caption().muted();
            for warning in warnings {
                text(warning).caption().color(palette().danger).fill_width();
            }
        }
        .gap(Space::TIGHT)
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}
