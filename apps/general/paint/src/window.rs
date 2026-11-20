use std::path::PathBuf;

use rikka_app::prelude::*;

use crate::paint::Paint;
use crate::tab::Tab;
use crate::{caption, view};

const SIZE: (f32, f32) = (1280.0, 820.0);
const SMALLEST: (f32, f32) = (720.0, 480.0);

pub(crate) fn opening(paths: Vec<PathBuf>) -> Window<Paint> {
    described(Window::new(
        move |sender: Sender<Paint>| Paint::new(sender, paths),
        view::view,
    ))
}

pub(crate) fn holding(tab: Tab) -> Window<Paint> {
    described(Window::new(
        move |sender: Sender<Paint>| Paint::holding(sender, tab),
        view::view,
    ))
}

fn described(window: Window<Paint>) -> Window<Paint> {
    window
        .title(tr!("paint.title"))
        .size(SIZE.0, SIZE.1)
        .min_size(SMALLEST.0, SMALLEST.1)
        .caption(caption::bar)
        .on_start(Paint::start)
        .on_focus(Paint::focused)
        .on_close_request(Paint::ask_to_close_window)
}
