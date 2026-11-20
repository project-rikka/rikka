use std::sync::Arc;

use rikka_compositor_desktop::{WindowId, WindowKind};
use rikka_compositor_geometry::{Logical, Size};

use crate::context::WindowsCtx;

fn extent(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

impl WindowsCtx<'_> {
    pub(super) fn client_limits(
        &mut self,
        window: WindowId,
    ) -> (Size<i32, Logical>, Size<i32, Logical>) {
        let Some(kind) = self
            .windows
            .desktop
            .window(window)
            .map(|record| record.kind())
        else {
            return (Size::zero(), Size::zero());
        };
        match kind {
            WindowKind::Xdg(toplevel) => self
                .shell
                .toplevel(toplevel)
                .map_or((Size::zero(), Size::zero()), |state| {
                    (state.min_size(), state.max_size())
                }),
            WindowKind::X11(x11) => self.x11.get().and_then(|xwm| xwm.window(x11)).map_or(
                (Size::zero(), Size::zero()),
                |state| {
                    let hints = state.size_hints;
                    let size = |size: Option<rikka_compositor_xwm::Size>| {
                        size.map_or(Size::zero(), |size| {
                            Size::new(extent(size.width), extent(size.height))
                        })
                    };
                    (size(hints.min_size()), size(hints.max))
                },
            ),
        }
    }

    pub(crate) fn window_title(&mut self, window: WindowId) -> Arc<str> {
        let Some(kind) = self
            .windows
            .desktop
            .window(window)
            .map(|record| record.kind())
        else {
            return Arc::from("");
        };
        let (title, app_id) = match kind {
            WindowKind::Xdg(toplevel) => {
                self.shell.toplevel(toplevel).map_or((None, None), |state| {
                    (
                        state.title().map(str::to_owned),
                        state.app_id().map(str::to_owned),
                    )
                })
            }
            WindowKind::X11(x11) => self
                .x11
                .get()
                .and_then(|xwm| xwm.window(x11))
                .map_or((None, None), |state| {
                    (Some(state.title.clone()), Some(state.class.clone()))
                }),
        };
        let title = title.filter(|title| !title.is_empty());
        Arc::from(title.or(app_id).unwrap_or_default())
    }
}
