use rikka_compositor_desktop::{Frame, Placement, WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_xwm::{Geometry, WindowProperty, XWindowId, XwmEvent};

use crate::context::{WindowsCtx, windows_ref};
use crate::windows::{Decoration, Enlargement, XStacking, frame_extents};

fn clamp(extent: u32) -> i32 {
    i32::try_from(extent).unwrap_or(i32::MAX).max(1)
}

fn rect_of(geometry: Geometry) -> Rect<i32, Logical> {
    Rect::new(
        Point::new(geometry.x, geometry.y),
        Size::new(clamp(geometry.width), clamp(geometry.height)),
    )
}

impl WindowsCtx<'_> {
    fn x11_configure_request(
        &mut self,
        window: XWindowId,
        width: Option<u32>,
        height: Option<u32>,
    ) {
        let Some(id) = self.windows.desktop.window_for_x11(window) else {
            return;
        };
        let Some((standing, frame)) = self
            .windows
            .desktop
            .window(id)
            .map(|record| (record.geometry(), record.frame()))
        else {
            return;
        };
        let managed_size = self.windows.is_tiled(id) || self.windows.enlargements(id).any();
        let size = match managed_size {
            true => standing.size,
            false => {
                let content = frame.content_size(standing.size);
                frame.outer_size(Size::new(
                    width.map_or(content.w, clamp),
                    height.map_or(content.h, clamp),
                ))
            }
        };
        self.stand(id, Rect::new(standing.loc, size));
    }

    fn x11_stands(&mut self, window: XWindowId, geometry: Geometry, loose_only: bool) {
        let Some(id) = self.windows.desktop.window_for_x11(window) else {
            return;
        };
        let Some(record) = self.windows.desktop.window_mut(id) else {
            return;
        };
        if loose_only && !record.unmanaged() {
            return;
        }
        let outer = record.frame().outer(rect_of(geometry));
        record.set_geometry(outer);
        self.desktop_changed();
    }

    fn x11_frame_extents_requested(&mut self, window: XWindowId) {
        let frame = match self.windows.desktop.window_for_x11(window) {
            Some(id) => self
                .windows
                .desktop
                .window(id)
                .map_or(Frame::NONE, |record| record.frame()),
            None => self
                .x11
                .get()
                .and_then(|xwm| xwm.window(window))
                .map_or(Frame::NONE, |state| {
                    Decoration::of_x11(state).frame(Placement::Floating, true)
                }),
        };
        self.x11.set_frame_extents(window, frame_extents(frame));
    }

    fn x11_enlargement(&mut self, window: XWindowId, mode: Enlargement, large: bool) {
        let Some(id) = self.windows.desktop.window_for_x11(window) else {
            return;
        };
        match large {
            true => self.enlarge(id, mode, None),
            false => self.shrink(id, mode),
        }
    }

    pub(crate) fn x11_event(&mut self, event: XwmEvent) {
        match event {
            XwmEvent::MapRequest { window } => self.arrive_x11(window),
            XwmEvent::Mapped { window } => {
                let loose = self
                    .x11
                    .get()
                    .and_then(|xwm| xwm.window(window))
                    .is_some_and(|state| state.override_redirect);
                if loose && self.windows.desktop.window_for_x11(window).is_none() {
                    self.arrive_x11(window);
                }
            }
            XwmEvent::Unmapped { window } | XwmEvent::Destroyed { window } => {
                if let Some(id) = self.windows.desktop.window_for_x11(window) {
                    self.depart(id);
                }
            }
            XwmEvent::SurfaceSerial { window, serial } => {
                if let Some(surface) = self.shell.surface_for_xwayland_serial(serial) {
                    self.associate_x11(window, surface);
                }
            }
            XwmEvent::ConfigureRequest {
                window,
                width,
                height,
                ..
            } => self.x11_configure_request(window, width, height),
            XwmEvent::ConfigureAccepted {
                window, geometry, ..
            } => self.x11_stands(window, geometry, false),
            XwmEvent::Configured { window, geometry } => self.x11_stands(window, geometry, true),
            XwmEvent::PropertyChanged { window, property } => {
                let Some(id) = self.windows.desktop.window_for_x11(window) else {
                    return;
                };
                match property {
                    WindowProperty::Title | WindowProperty::Class => {
                        self.refresh_foreign(id);
                        self.retitle(id);
                    }
                    WindowProperty::Decorations
                    | WindowProperty::WindowType
                    | WindowProperty::SizeHints => self.reframe(id),
                    _ => {}
                }
            }
            XwmEvent::FrameExtentsRequest { window } => self.x11_frame_extents_requested(window),
            XwmEvent::MaximizeRequest { window, maximized } => {
                self.x11_enlargement(window, Enlargement::Maximized, maximized);
            }
            XwmEvent::FullscreenRequest { window, fullscreen } => {
                self.x11_enlargement(window, Enlargement::Fullscreen, fullscreen);
            }
            XwmEvent::MinimizeRequest { window, minimized } => {
                if let Some(id) = self.windows.desktop.window_for_x11(window) {
                    match minimized {
                        true => self.minimize_window(id),
                        false => self.reveal_window(id),
                    }
                }
            }
            XwmEvent::ActivateRequest { window } => {
                if let Some(id) = self.windows.desktop.window_for_x11(window) {
                    self.reveal_window(id);
                }
            }
            XwmEvent::SelectionOffered { kind, mime_types } => {
                self.x11_selection_offered(kind, mime_types);
            }
            XwmEvent::SelectionCleared { kind } => self.x11_selection_cleared(kind),
            XwmEvent::SendSelection {
                kind,
                mime_type,
                fd,
            } => self.x11_selection_requested(kind, mime_type, fd),
            XwmEvent::MoveRequest { window, .. } => self.x11_move_requested(window),
            XwmEvent::ResizeRequest { window, edge, .. } => {
                self.x11_resize_requested(window, edge.into());
            }
            XwmEvent::AboveRequest { .. } => {}
            XwmEvent::X11DragOwner { owned } => self.x11_drag_owned(owned),
            XwmEvent::X11DragEntered { mime_types } => self.x11_drag_entered(mime_types),
            XwmEvent::X11DragLeft => self.x11_drag_left(),
            XwmEvent::X11DragDropped => self.x11_drag_dropped(),
            XwmEvent::DragStatus { accepted, action } => self.x11_drag_status(accepted, action),
            XwmEvent::DragFinished { accepted, action } => {
                self.x11_drag_finished(accepted, action);
            }
        }
    }

    pub(crate) fn xwayland_associated(&mut self, surface: SurfaceId, serial: u64) {
        let window = self.x11.get().and_then(|xwm| xwm.window_for_serial(serial));
        if let Some(window) = window {
            self.associate_x11(window, surface);
        }
    }

    fn associate_x11(&mut self, window: XWindowId, surface: SurfaceId) {
        let Some(id) = self.windows.desktop.window_for_x11(window) else {
            return;
        };
        if let Err(error) = self.windows.desktop.associate(id, Some(surface)) {
            return tracing::debug!("cannot associate X11 window with surface: {error}");
        }
        if windows_ref!(self).focused_window().is_none()
            && self
                .windows
                .desktop
                .window(id)
                .is_some_and(|record| !record.unmanaged())
        {
            self.focus_window(Some(id));
        }
        self.desktop_changed();
    }

    pub(crate) fn forget_x11_windows(&mut self) {
        let x11: Vec<WindowId> = self
            .windows
            .desktop
            .windows()
            .filter(|(_, record)| matches!(record.kind(), WindowKind::X11(_)))
            .map(|(id, _)| id)
            .collect();
        for window in x11 {
            self.depart(window);
        }
        self.windows.stacking = XStacking::default();
        self.x11_drags_gone();
    }
}
