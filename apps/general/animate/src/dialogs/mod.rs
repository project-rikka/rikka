mod export;
mod import;
mod progress;
mod size;

use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::animate::Animate;
use crate::output::Sheet;

pub(crate) const STAGE: &str = "animate-stage";
const CAPTION_WIDTH: f32 = 96.0;

pub(crate) fn is_open(animate: &Animate) -> bool {
    animate.output().sheet().is_some() || animate.job().is_some() || animate.choosing().is_some()
}

pub(crate) fn view(animate: &Animate) -> Vec<Element> {
    if let Some(job) = animate.job() {
        return vec![outside(None), progress::sheet(job)];
    }
    match animate.output().sheet() {
        Some(Sheet::Size(sizing)) => vec![outside(Some(close())), size::sheet(sizing)],
        Some(Sheet::Export(exporting)) => {
            vec![
                outside(Some(close())),
                export::sheet(exporting, animate.transport().last_frame()),
            ]
        }
        None => match animate.choosing() {
            Some(choosing) => vec![
                outside(None),
                import::sheet(choosing, animate.subject_figure(), animate.can_settle()),
            ],
            None => Vec::new(),
        },
    }
}

fn close() -> Handler {
    (|animate: &mut Animate| animate.output_mut().close()).into_handler()
}

fn outside(dismiss: Option<Handler>) -> Element {
    Element::new(Outside { dismiss })
        .position(0.0, 0.0)
        .fill()
        .build()
}

struct Outside {
    dismiss: Option<Handler>,
}

impl Widget for Outside {
    type State = ();

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.set_handled();
            if let Some(dismiss) = &self.dismiss {
                ctx.call(dismiss, ());
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.dismiss.is_some() == other.dismiss.is_some()
    }

    fn debug_name(&self) -> &'static str {
        "Outside"
    }
}

impl std::fmt::Debug for Outside {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outside").finish_non_exhaustive()
    }
}

fn labelled(caption: Label, control: impl View) -> impl View {
    row! {
        text(caption).caption().muted().width(CAPTION_WIDTH).shrink(0.0);
        control;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
