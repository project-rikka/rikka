use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_settings::session::color_picker;
use shell_kit::{EXIT_DURATION, Feature, KeyboardMode, Recents, Shell, SurfaceDesc};

use crate::editor::{self, Editor};
use crate::picker::{self, Sheet};
use crate::service::{ACTIVATE, ColorPickerService};
use crate::shared::Shared;
use crate::{CAPABILITY, HISTORY, preferences};

#[derive(Debug, Default)]
pub struct ColorPicker;

impl ColorPicker {
    #[must_use]
    pub fn new() -> ColorPicker {
        ColorPicker
    }
}

impl Feature for ColorPicker {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(color_picker::schema());
        let (shell, preferences) = preferences::follow(shell);
        let shared = Shared::new(shell.seam(), preferences, Recents::read(HISTORY));
        let service = ColorPickerService::new(shared.clone());
        let editor = shared.editor.clone();
        let shell = shell.service(service);

        shell
            .on_shortcut(Shortcut::ColorPicker, Request::new(CAPABILITY, ACTIVATE))
            .surface(
                SurfaceDesc::per_output(
                    {
                        let shared = shared.clone();
                        move |place| Sheet::new(shared.clone(), &place)
                    },
                    picker::view,
                )
                .name("color-picker")
                .overlay()
                .keyboard(KeyboardMode::Exclusive)
                .presence(shared.picking.clone()),
            )
            .surface(
                SurfaceDesc::new(move || Editor::new(shared.clone()), editor::view)
                    .name("color-editor")
                    .modal(editor, EXIT_DURATION),
            )
    }
}
