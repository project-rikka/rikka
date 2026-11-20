#![cfg_attr(windows, windows_subsystem = "windows")]

mod animate;
mod arrival;
mod assistant;
mod bar;
mod caption;
mod command;
mod dialogs;
mod output;
mod preferences;
mod view;
mod wording;

use rikka_app::prelude::*;

use crate::animate::Animate;
use crate::preferences::Preferences;

const SIZE: (f32, f32) = (1360.0, 900.0);
const SMALLEST: (f32, f32) = (760.0, 560.0);
const SPIRV_VALIDATION: (&str, &str) = ("SLANG_RUN_SPIRV_VALIDATION", "1");

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    unsafe { std::env::set_var(SPIRV_VALIDATION.0, SPIRV_VALIDATION.1) };
    App::new("rikka-animate")
        .draws_3d()
        .computes_in_background()
        .i18n(
            rikka_i18n::embedded!()
                .embedded(rikka_timeline::CATALOGS)
                .embedded(animate_ui::CATALOGS)
                .embedded(animate_timeline::CATALOGS)
                .embedded(animate_viewport::CATALOGS)
                .embedded(animate_inspector::CATALOGS)
                .embedded(animate_shader_editor::CATALOGS)
                .embedded(animate_export::CATALOGS),
        )
        .windows(windows)
        .run()
}

fn windows(open: LaunchRequest) -> Vec<Window<Animate>> {
    let opening = open.paths();
    vec![
        Window::new(
            move |sender: Sender<Animate>| Animate::new(opening, Preferences::open(), sender),
            view::view,
        )
        .title(tr!("animate.title"))
        .size(SIZE.0, SIZE.1)
        .min_size(SMALLEST.0, SMALLEST.1)
        .caption(caption::bar)
        .on_start(|animate: &mut Animate| animate.start())
        .on_focus(|animate: &mut Animate| animate.focused()),
    ]
}
