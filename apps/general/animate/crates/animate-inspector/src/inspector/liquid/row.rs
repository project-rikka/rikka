use rikka_app::prelude::*;

pub(super) fn picker(chosen: Option<usize>, options: Vec<Label>) -> Combo {
    combo(chosen)
        .options(options)
        .focusless()
        .control_size(ControlSize::Small)
}
