use rikka_compositor_desktop::WindowId;
use rikka_compositor_output::{CaptureSource, SessionInfo};
use rikka_compositor_types::OutputId;

use crate::context::CaptureRef;
use crate::render::DragIcon;
use crate::render::{Composed, Framing, View};
use crate::windows::drawn_on;

#[derive(Debug)]
pub(super) struct Subject {
    pub(super) view: Option<View>,
    pub(super) framing: Framing,
    pub(super) composed: Composed,
    pub(super) icon: Option<DragIcon>,
    pub(super) screen: Option<OutputId>,
}

impl CaptureRef<'_> {
    pub(super) fn capture_subject(
        &self,
        output: OutputId,
        info: SessionInfo<WindowId>,
    ) -> Option<Subject> {
        let pointer = info.cursor.is_some();
        match info.source {
            CaptureSource::Output(published) => Some(Subject {
                view: self.outputs.get(output).and_then(View::from_output),
                framing: match pointer {
                    true => Framing::Cursor,
                    false => Framing::Screen {
                        cursor: info.paint_cursors,
                    },
                },
                composed: self.scenes.composed(output).cloned().unwrap_or_else(|| {
                    let mut drawn = Vec::new();
                    drawn_on(self.windows, self.surfaces, published, &mut drawn);
                    Composed {
                        drawn,
                        ..Composed::default()
                    }
                }),
                icon: self.selection.icon(self.input.seats()),
                screen: Some(output),
            }),
            CaptureSource::Toplevel(window) => Some(Subject {
                view: Some(self.window_view(window)?),
                framing: match pointer {
                    true => Framing::Cursor,
                    false => Framing::Window {
                        cursor: info.paint_cursors,
                    },
                },
                composed: Composed {
                    drawn: self.windows.window_drawn(window),
                    ..Composed::default()
                },
                icon: None,
                screen: None,
            }),
        }
    }
}
