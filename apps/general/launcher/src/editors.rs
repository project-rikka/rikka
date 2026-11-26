use launcher_instance::Window;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use crate::launcher::Launcher;
use crate::preferences::{MIN_MEMORY, total_memory};

const MEMORY_STEP: f32 = 256.0;
const NUMBER: f32 = 120.0;
const MEMORY_LABEL: f32 = 90.0;

pub(crate) fn memory_editor(megabytes: u32, change: fn(&mut Launcher, f32)) -> Element {
    let floor = MIN_MEMORY as f32;
    row! {
        slider(megabytes as f32)
            .range(floor..=(total_memory() as f32).max(floor * 2.0))
            .step(MEMORY_STEP)
            .on_commit(change)
            .grow(1.0);
        text(format!("{megabytes} MB")).label().no_wrap().width(Length::px(MEMORY_LABEL));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(crate) fn window_editor(window: Window, change: fn(&mut Launcher, Window)) -> Element {
    row! {
        number_field(window.width as f32)
            .range(320.0..=7680.0)
            .unit("px")
            .on_commit(move |launcher: &mut Launcher, value: f32| {
                change(launcher, Window { width: value.round() as u32, ..window });
            })
            .width(Length::px(NUMBER));
        text("×").label().muted();
        number_field(window.height as f32)
            .range(240.0..=4320.0)
            .unit("px")
            .on_commit(move |launcher: &mut Launcher, value: f32| {
                change(launcher, Window { height: value.round() as u32, ..window });
            })
            .width(Length::px(NUMBER));
        spacer().grow(1.0);
        text(tr!("launcher.options.fullscreen")).label();
        switch(window.fullscreen).on_toggle(move |launcher: &mut Launcher, on: bool| {
            change(launcher, Window { fullscreen: on, ..window });
        });
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}
