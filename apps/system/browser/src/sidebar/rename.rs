use rikka_app::prelude::*;

use crate::shell::Shell;

pub(crate) fn field(shell: &Shell) -> Element {
    text_field(shell.sidebar().draft())
        .control_size(ControlSize::Small)
        .claims_focus(true)
        .on_submit(|shell: &mut Shell| shell.finish_renaming())
        .on_blur(|shell: &mut Shell| shell.finish_renaming())
        .on_key(|shell: &mut Shell, key: Key, _: Modifiers| match key {
            Key::Escape => {
                shell.abandon_renaming();
                true
            }
            _ => false,
        })
        .grow(1.0)
        .min_width(0.0)
        .any()
}
