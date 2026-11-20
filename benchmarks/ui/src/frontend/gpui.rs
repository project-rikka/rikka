macro_rules! headless_frontend {
    ($frontend:ident, $gpui:ident, $wgpu:ident, $views:ident) => {
        use std::sync::Arc;

        use crate::{Frontend, Screen};
        use ui_scene::{WINDOW_HEIGHT, WINDOW_WIDTH};
        use $gpui::{
            AnyWindowHandle, AppContext, Context, Entity, HeadlessAppContext, IntoElement, Render,
            Window, px, size,
        };

        const FALLBACK_FONT: &str = "IBM Plex Sans";

        struct Shown {
            screen: Screen,
        }

        impl Render for Shown {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                match &self.screen {
                    Screen::Table(table) => $views::table(table).into_any_element(),
                    Screen::Grid(grid) => $views::grid(grid).into_any_element(),
                }
            }
        }

        pub struct $frontend {
            cx: HeadlessAppContext,
            shown: Entity<Shown>,
        }

        impl Frontend for $frontend {
            type Host = HeadlessAppContext;

            fn host() -> HeadlessAppContext {
                HeadlessAppContext::new(Arc::new($wgpu::CosmicTextSystem::new(FALLBACK_FONT)))
            }

            fn open(mut cx: HeadlessAppContext, screen: Screen) -> $frontend {
                let mut shown = None;
                let window: AnyWindowHandle = cx
                    .open_window(size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), |_, cx| {
                        let entity = cx.new(|_| Shown { screen });
                        shown = Some(entity.clone());
                        entity
                    })
                    .expect("headless window opens")
                    .into();
                cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
                    .expect("headless window is open");
                $frontend {
                    cx,
                    shown: shown.expect("the window built its root view"),
                }
            }

            fn update(&mut self, change: impl FnOnce(&mut Screen)) {
                let shown = self.shown.clone();
                self.cx.update(|cx| {
                    shown.update(cx, |shown, cx| {
                        change(&mut shown.screen);
                        cx.notify();
                    });
                });
            }
        }

        impl std::fmt::Debug for $frontend {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct(stringify!($frontend))
                    .finish_non_exhaustive()
            }
        }
    };
}

mod fast {
    headless_frontend!(GpuiFast, gpui_fast, gpui_fast_wgpu, ui_gpui_fast);
}

mod upstream {
    headless_frontend!(Gpui, gpui_pre, gpui_pre_wgpu, ui_gpui_upstream);
}

pub use fast::GpuiFast;
pub use upstream::Gpui;
