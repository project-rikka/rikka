use rikka_compositor_desktop::Tags;
use rikka_compositor_protocols::xdg_toplevel::ResizeEdge;
use rikka_compositor_shell::{ToplevelEvent, ToplevelId};
use rikka_compositor_tiling::Edges;

use crate::context::{WindowsCtx, windows_ref};
use crate::windows::Enlargement;

fn grabbed(edge: ResizeEdge) -> Edges {
    match edge {
        ResizeEdge::None => Edges::empty(),
        ResizeEdge::Top => Edges::TOP,
        ResizeEdge::Bottom => Edges::BOTTOM,
        ResizeEdge::Left => Edges::LEFT,
        ResizeEdge::Right => Edges::RIGHT,
        ResizeEdge::TopLeft => Edges::TOP_LEFT,
        ResizeEdge::TopRight => Edges::TOP_RIGHT,
        ResizeEdge::BottomLeft => Edges::BOTTOM_LEFT,
        ResizeEdge::BottomRight => Edges::BOTTOM_RIGHT,
    }
}

impl WindowsCtx<'_> {
    fn maximize_requested(
        &mut self,
        toplevel: ToplevelId,
        mode: Enlargement,
        large: bool,
        onto: Option<rikka_compositor_types::OutputId>,
    ) {
        match self.windows.desktop.window_for_toplevel(toplevel) {
            Some(window) if large => self.enlarge(window, mode, onto),
            Some(window) => self.shrink(window, mode),
            None => self.arriving_enlarged(toplevel, mode, large),
        }
    }

    pub(crate) fn toplevel_event(&mut self, toplevel: ToplevelId, event: ToplevelEvent) {
        let window = self.windows.desktop.window_for_toplevel(toplevel);
        match event {
            ToplevelEvent::Move { seat, serial } => {
                if let Some(window) = window {
                    self.move_requested(window, &seat, serial);
                }
            }
            ToplevelEvent::Resize {
                seat,
                serial,
                edges,
            } => {
                if let Some(window) = window {
                    self.resize_requested(window, &seat, serial, grabbed(edges));
                }
            }
            ToplevelEvent::InitialCommit => self.plan_arrival(toplevel),
            ToplevelEvent::Mapped => self.arrive_toplevel(toplevel),
            ToplevelEvent::Unmapped | ToplevelEvent::Destroyed => {
                self.departed_toplevel(toplevel);
            }
            ToplevelEvent::Configured { .. } => self.flush_configures(),
            ToplevelEvent::Title | ToplevelEvent::AppId => {
                if let Some(window) = window {
                    self.refresh_foreign(window);
                    self.retitle(window);
                }
            }
            ToplevelEvent::Parent | ToplevelEvent::Modal => self.retag(toplevel),
            ToplevelEvent::Decoration => match window {
                Some(window) => {
                    self.reframe(window);
                    self.windows.queue_configure(window);
                }
                None => self.decoration_chosen_early(toplevel),
            },
            ToplevelEvent::Maximize | ToplevelEvent::Unmaximize => {
                let large = matches!(event, ToplevelEvent::Maximize);
                self.maximize_requested(toplevel, Enlargement::Maximized, large, None);
            }
            ToplevelEvent::Fullscreen { output } => {
                let onto = output.and_then(|output| self.outputs.wayland().output_of(&output));
                self.maximize_requested(toplevel, Enlargement::Fullscreen, true, onto);
            }
            ToplevelEvent::Unfullscreen => {
                self.maximize_requested(toplevel, Enlargement::Fullscreen, false, None);
            }
            ToplevelEvent::Minimize => {
                if let Some(window) = window {
                    self.minimize_window(window);
                }
            }
            ToplevelEvent::Created
            | ToplevelEvent::Icon
            | ToplevelEvent::Tag
            | ToplevelEvent::Description
            | ToplevelEvent::WindowMenu { .. } => {}
            ToplevelEvent::DragAttached { offset, .. } => self.drag_attached(toplevel, offset),
        }
    }

    fn retag(&mut self, toplevel: ToplevelId) {
        let Some(window) = self.windows.desktop.window_for_toplevel(toplevel) else {
            return;
        };
        let Some(state) = self.shell.toplevel(toplevel) else {
            return;
        };
        let mut tags = Tags::empty();
        tags.set(Tags::DIALOG, state.parent().is_some());
        tags.set(Tags::MODAL, state.modal());
        if let Some(record) = self.windows.desktop.window_mut(window) {
            record.set_tags(tags);
        }
        if tags.contains(Tags::DIALOG) && self.windows.is_tiled(window) {
            self.forget_tile(window);
            self.restore_floating(window);
        }
        if let Some(focused) = windows_ref!(self).focused_window()
            && windows_ref!(self).modal_blocker(focused).is_some()
        {
            self.focus_window(Some(focused));
        }
    }
}
