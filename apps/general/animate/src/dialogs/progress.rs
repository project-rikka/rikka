use animate_export::{Job, Phase, Summary};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::STAGE;
use crate::animate::Animate;

const WIDTH: f32 = 520.0;
const PREVIEW_HEIGHT: f32 = 270.0;

pub(super) fn sheet(job: &Job) -> Element {
    let title = title(job.phase());
    let frame = match job.phase() {
        Phase::Failed(error) => dialog(
            title,
            text(animate_ui::error_chain(error))
                .color(palette().danger)
                .fill_width(),
        )
        .accept(tr!("animate.export.close"), Animate::close_export),
        Phase::Done(summary) => dialog(title, done(job, summary))
            .aside(
                button(text(tr!("animate.export.show")))
                    .ghost()
                    .on_press(Animate::show_export),
            )
            .action(button(text(tr!("animate.export.play"))).on_press(Animate::play_export))
            .accept(tr!("animate.export.close"), Animate::close_export),
        Phase::Preparing | Phase::Rendering | Phase::Finishing | Phase::Cancelling => {
            dialog(title, running(job)).action(
                button(text(tr!("animate.output.cancel")))
                    .ghost()
                    .enabled(job.can_cancel())
                    .on_press(Animate::cancel_export),
            )
        }
    };
    frame
        .on_dismiss(|animate: &mut Animate| {
            animate.cancel_export();
            animate.close_export();
        })
        .width(WIDTH)
        .keyed(("animate-export-progress", 0))
        .centered_on(STAGE)
        .build()
}

fn title(phase: &Phase) -> Label {
    match phase {
        Phase::Preparing => tr!("animate.export.preparing"),
        Phase::Rendering => tr!("animate.export.rendering"),
        Phase::Finishing => tr!("animate.export.finishing"),
        Phase::Cancelling => tr!("animate.export.cancelling"),
        Phase::Done(_) => tr!("animate.export.done"),
        Phase::Failed(_) => tr!("animate.export.failed"),
    }
}

fn screen(job: &Job) -> impl View {
    let previews = job
        .preview()
        .cloned()
        .map(|preview| image(preview).fit(ImageFit::Contain).fill().build());
    container(column_of(previews.into_iter().collect::<Vec<Element>>()).fill())
        .background(Color::BLACK)
        .radius(Corner::Item)
        .fill_width()
        .height(PREVIEW_HEIGHT)
}

fn running(job: &Job) -> impl View {
    let steady = matches!(job.phase(), Phase::Rendering);
    let bar = match steady {
        true => progress(job.fraction()),
        false => progress_indeterminate(),
    };
    let mut facts = vec![tr!(
        "animate.export.counted",
        frames = job.frames(),
        total = job.total()
    )];
    if let Some(pace) = job.pace().filter(|_| steady) {
        facts.push(tr!(
            "animate.export.pace",
            pace = format!("{pace:.1}").as_str()
        ));
    }
    if let Some(remaining) = job.remaining().filter(|_| steady) {
        facts.push(tr!(
            "animate.export.remaining",
            duration = rikka_format::format_duration(remaining).as_str()
        ));
    }
    column! {
        screen(job);
        bar.fill_width();
        row! {
            text(joined(&facts)).caption().grow(1.0);
            if let Some(backend) = job.backend() {
                text(animate_export::backend_label(backend)).caption().muted();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn done(job: &Job, summary: &Summary) -> impl View {
    let name = summary
        .path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let facts = [
        Label::from(rikka_format::format_duration(summary.duration)),
        tr!("animate.export.frame_count", frames = summary.frames),
        animate_export::backend_label(summary.backend),
    ];
    column! {
        screen(job);
        column! {
            text(name).semibold().no_wrap();
            text(joined(&facts)).caption().muted();
        }
        .gap(Space::TIGHT)
        .fill_width();
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn joined(facts: &[Label]) -> String {
    facts
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(" · ")
}
