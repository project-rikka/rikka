use rikka_compositor_selection::{
    SelectionEvent, SelectionHost, Selections, SendDrag, SendSelection,
};
use rikka_compositor_surface::SurfaceId;

use crate::context::input_ctx;
use crate::context::windows_ctx;
use crate::state::Compositor;

impl SelectionHost for Compositor {
    fn selections(&mut self) -> &mut Selections {
        &mut self.selection.selections
    }

    fn send_selection(&mut self, request: SendSelection) {
        input_ctx!(self).x11_send_selection(request);
    }

    fn send_drag(&mut self, request: SendDrag) {
        input_ctx!(self).x11_drag_request(request);
    }

    fn foreign_drag_target(&mut self, surface: SurfaceId) -> bool {
        self.windows.x11_window_of(surface).is_some()
    }

    fn selection_event(&mut self, event: SelectionEvent) {
        match event {
            SelectionEvent::Changed { seat, kind } => {
                input_ctx!(self).selection_changed(seat, kind)
            }
            SelectionEvent::DragStarted { .. }
            | SelectionEvent::DragIconChanged { .. }
            | SelectionEvent::DragEnded { .. } => {
                self.frames.request_all();
            }
            SelectionEvent::DragAccepted { seat } => {
                let carries = self
                    .selection
                    .selections
                    .drag(seat)
                    .is_some_and(|drag| drag.toplevel.is_some());
                if carries {
                    self.frames.request_all();
                }
            }
            SelectionEvent::DragToplevelMoved { toplevel, at, .. } => {
                input_ctx!(self).dragged_toplevel_moved(toplevel, at);
            }
            SelectionEvent::DragToplevelReleased { toplevel, .. } => {
                input_ctx!(self).drag_released(toplevel);
            }
            SelectionEvent::ForeignDrag { offer, event, .. } => {
                windows_ctx!(self).foreign_dragged(offer, event)
            }
            SelectionEvent::CompositorDrag { source, event, .. } => {
                input_ctx!(self).compositor_dragged(source, event);
            }
        }
    }
}
