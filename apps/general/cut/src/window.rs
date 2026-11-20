use std::path::PathBuf;

use rikka_app::prelude::*;

use crate::cut::Cut;
use crate::{caption, view};

const SIZE: (f32, f32) = (1440.0, 900.0);
const SMALLEST: (f32, f32) = (800.0, 560.0);

pub(crate) fn opening(paths: Vec<PathBuf>) -> Window<Cut> {
    Window::new(
        move |sender: Sender<Cut>| Cut::new(paths, sender),
        view::view,
    )
    .title(tr!("cut.title"))
    .size(SIZE.0, SIZE.1)
    .min_size(SMALLEST.0, SMALLEST.1)
    .caption(caption::bar)
    .on_start(Cut::start)
    .on_close_request(Cut::ask_to_close)
}
