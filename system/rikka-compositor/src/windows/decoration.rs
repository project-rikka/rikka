use std::sync::Arc;

use rikka_compositor_caption::{Button, Caption};
use rikka_compositor_desktop::{Frame, Placement, WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Rect, Size};
use rikka_compositor_shell::{DecorationMode, DecorationPreference, ToplevelConfigure};
use rikka_compositor_xwm::{FrameExtents, WindowType, XWindow};

use super::caption_pointer::CaptionPointer;
use crate::context::WindowsCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decoration {
    Client,
    Server,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decorated {
    pub(crate) title: Arc<str>,
    pub(crate) resizable: bool,
}

impl Decoration {
    pub(crate) fn of_xdg(preference: DecorationPreference) -> Self {
        match preference {
            DecorationPreference::Prefers(DecorationMode::ServerSide)
            | DecorationPreference::Unset => Self::Server,
            DecorationPreference::Prefers(DecorationMode::ClientSide)
            | DecorationPreference::Absent => Self::Client,
        }
    }

    pub(crate) fn of_x11(window: &XWindow) -> Self {
        let framed = matches!(
            window.window_type(),
            WindowType::Normal | WindowType::Dialog | WindowType::Utility | WindowType::Toolbar
        );
        match !window.override_redirect && window.decorated && framed {
            true => Self::Server,
            false => Self::Client,
        }
    }

    pub(crate) fn mode(self) -> DecorationMode {
        match self {
            Self::Client => DecorationMode::ClientSide,
            Self::Server => DecorationMode::ServerSide,
        }
    }

    pub(crate) fn frame(self, placement: Placement, resizable: bool) -> Frame {
        match (self, placement) {
            (Self::Client, _) | (Self::Server, Placement::Fullscreen) => Frame::NONE,
            (Self::Server, Placement::Floating) => Frame {
                title: Caption::BAR,
                reach: if resizable { Caption::REACH } else { 0 },
            },
            (Self::Server, Placement::Tiled | Placement::Maximized) => Frame {
                title: Caption::BAR,
                reach: 0,
            },
        }
    }
}

pub(crate) fn frame_extents(frame: Frame) -> FrameExtents {
    FrameExtents {
        top: u32::try_from(frame.title).unwrap_or(0),
        ..FrameExtents::default()
    }
}

pub(crate) fn client_configure(
    outer: ToplevelConfigure,
    frame: Frame,
    decoration: Decoration,
) -> ToplevelConfigure {
    ToplevelConfigure {
        size: outer.size.map(|size| match size.w > 0 && size.h > 0 {
            true => frame.content_size(size),
            false => size,
        }),
        bounds: outer.bounds.map(|size| frame.content_size(size)),
        decoration: Some(decoration.mode()),
        ..outer
    }
}

fn resizable((least, most): (Size<i32, Logical>, Size<i32, Logical>)) -> bool {
    let fixed = |least: i32, most: i32| most > 0 && least >= most;
    !(fixed(least.w, most.w) && fixed(least.h, most.h))
}

fn outer_limits(
    (least, most): (Size<i32, Logical>, Size<i32, Logical>),
    frame: Frame,
) -> (Size<i32, Logical>, Size<i32, Logical>) {
    let grown = |size: Size<i32, Logical>| match size.h > 0 {
        true => frame.outer_size(size),
        false => size,
    };
    (grown(least), grown(most))
}

impl WindowsCtx<'_> {
    pub(crate) fn decoration_of(&self, kind: WindowKind) -> Decoration {
        match kind {
            WindowKind::Xdg(toplevel) => self
                .shell
                .toplevel(toplevel)
                .map_or(Decoration::Client, |state| {
                    Decoration::of_xdg(state.decoration())
                }),
            WindowKind::X11(x11) => self
                .x11
                .get()
                .and_then(|xwm| xwm.window(x11))
                .map_or(Decoration::Client, Decoration::of_x11),
        }
    }

    pub(crate) fn size_limits(
        &mut self,
        window: WindowId,
    ) -> (Size<i32, Logical>, Size<i32, Logical>) {
        let frame = self
            .windows
            .desktop
            .window(window)
            .map_or(Frame::NONE, |record| record.frame());
        outer_limits(self.client_limits(window), frame)
    }

    pub(crate) fn refresh_frame(&mut self, window: WindowId) -> bool {
        let Some(record) = self.windows.desktop.window(window) else {
            return false;
        };
        if record.unmanaged() {
            return false;
        }
        let (kind, placement) = (record.kind(), record.placement());
        let decoration = self.decoration_of(kind);
        let resizable = resizable(self.client_limits(window));
        let frame = decoration.frame(placement, resizable);
        let known = self
            .windows
            .desktop
            .policy(window)
            .and_then(|policy| policy.decorated.as_ref())
            .map(|decorated| decorated.title.clone());
        let decorated = (decoration == Decoration::Server).then(|| Decorated {
            title: known.unwrap_or_else(|| self.window_title(window)),
            resizable,
        });
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return false;
        };
        let was = record.frame();
        record.set_frame(frame);
        if policy.decorated != decorated {
            policy.decorated = decorated;
            self.redraw_window(window);
        }
        if was.title != frame.title
            && let WindowKind::X11(x11) = kind
        {
            self.x11.set_frame_extents(x11, frame_extents(frame));
        }
        was != frame
    }

    pub(crate) fn reframe(&mut self, window: WindowId) {
        if !self.refresh_frame(window) {
            return;
        }
        let Some((record, policy)) = self.windows.desktop.window_and_policy_mut(window) else {
            return;
        };
        match record.kind() {
            WindowKind::Xdg(_) => {
                if record.placement() == Placement::Floating && policy.configure.size.is_some() {
                    policy.configure.size = Some(record.geometry().size);
                }
                self.windows.queue_configure(window);
            }
            WindowKind::X11(_) => {
                let geometry = record.geometry();
                self.configure_x11(window, geometry);
            }
        }
        self.desktop_changed();
    }

    pub(crate) fn retitle(&mut self, window: WindowId) {
        if !self.windows.is_decorated(window) {
            return;
        }
        let title = self.window_title(window);
        let Some(decorated) = self
            .windows
            .desktop
            .policy_mut(window)
            .and_then(|policy| policy.decorated.as_mut())
        else {
            return;
        };
        if decorated.title != title {
            decorated.title = title;
            self.redraw_window(window);
        }
    }

    pub(crate) fn update_caption(&mut self, change: impl FnOnce(&mut CaptionPointer)) {
        let before = self.windows.captions.lights();
        change(&mut self.windows.captions);
        let after = self.windows.captions.lights();
        if before == after {
            return;
        }
        let windows: Vec<WindowId> = before
            .into_iter()
            .chain(after)
            .flatten()
            .map(|(window, _)| window)
            .collect();
        for window in windows {
            self.redraw_window(window);
        }
    }

    pub(crate) fn caption_button_clicked(&mut self, window: WindowId, button: Button) {
        match button {
            Button::Minimize => self.minimize_window(window),
            Button::Maximize | Button::Restore => self.toggle_maximized(window),
            Button::Close => self.close_window(window),
        }
    }

    pub(crate) fn configure_x11(&mut self, window: WindowId, rect: Rect<i32, Logical>) -> bool {
        let Some(record) = self.windows.desktop.window(window) else {
            return false;
        };
        let WindowKind::X11(x11) = record.kind() else {
            return false;
        };
        let content = record.frame().content(rect);
        self.x11.configure(x11, content)
    }
}
