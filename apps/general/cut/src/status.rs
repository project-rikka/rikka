use std::time::Instant;

use cut_project::Timecode;
use rikka_app::prelude::*;

use crate::command::Command;
use crate::cut::Cut;
use crate::transport::Transport;
use crate::wording;

const TEXT: f32 = 12.0;
const MODIFIED: &str = "• ";
const SPINNER: f32 = 14.0;

pub(crate) fn status(cut: &Cut) -> impl View {
    let mut name = cut.name();
    if cut.is_modified() {
        name.insert_str(0, MODIFIED);
    }
    let format = wording::format(cut.project().format());
    let settings = button(text(format.clone()).caption().no_wrap())
        .ghost()
        .small()
        .focusless()
        .enabled(Command::ProjectSettings.enabled(cut))
        .on_press(|cut: &mut Cut| Command::ProjectSettings.run(cut));
    let mut identity = Vec::new();
    if cut.is_busy() {
        identity.push(tool_item(spinner().extent(Length::px(SPINNER))));
    }
    identity.push(tool_item(text(name).caption().muted().no_wrap()));
    toolbar()
        .divided()
        .gap(Space::SNUG)
        .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
        .group(
            tool_group([tool_item(Clock {
                transport: cut.transport().clone(),
                end: cut.project().end(),
            })])
            .priority(ToolPriority::Pinned),
        )
        .group(tool_group([tool_item(settings).overflow([
            Command::ProjectSettings
                .entry_named(Label::from(format), cut)
                .icon(MaterialSymbol::TUNE),
        ])]))
        .spacer()
        .group(tool_group(identity).priority(ToolPriority::Low))
        .background(palette().surface)
}

#[derive(Debug)]
pub(crate) struct Clock {
    transport: Transport,
    end: i64,
}

impl Clock {
    fn label(&self, frame: i64) -> String {
        let rate = self.transport.rate();
        format!(
            "{} / {}    {frame} / {}",
            Timecode::from_frame(frame, rate),
            Timecode::from_frame(self.end, rate),
            self.end
        )
    }

    fn run(label: String) -> TextRun {
        TextRun::new(label, TEXT)
            .family(Family::Monospace)
            .no_wrap()
    }
}

impl Widget for Clock {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if self.transport.is_playing() {
            ctx.request_frame();
        }
        let frame = self.transport.exact(Instant::now()).floor() as i64;
        let run = Self::run(self.label(frame));
        let bounds = ctx.bounds();
        let height = measure_text(&run, None).height;
        let line = Rect::new(
            bounds.x(),
            bounds.center().y - height / 2.0,
            bounds.width(),
            height,
        );
        ctx.draw_text(line, run, palette().text);
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let widest = self
            .label(self.end)
            .replace(|digit: char| digit.is_ascii_digit(), "0");
        Some(MeasureSpec::text(Self::run(widest)))
    }

    fn same(&self, other: &Clock) -> bool {
        self.transport == other.transport && self.end == other.end
    }
}

impl View for Clock {
    fn build(self) -> Element {
        Element::new(self)
    }
}
