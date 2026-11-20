use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_ui::Inset;
use shell_kit::{
    Anchor, Feature, KeyboardMode, Layer, PresenceService, Shell, SurfaceDesc, TOGGLE,
};

use crate::board::Board;
use crate::keypad::Keypad;
use crate::{CAPABILITY, view};

const MARGIN: f32 = 16.0;

#[derive(Debug, Default)]
pub struct OnScreenKeyboard;

impl OnScreenKeyboard {
    #[must_use]
    pub fn new() -> OnScreenKeyboard {
        OnScreenKeyboard
    }
}

impl Feature for OnScreenKeyboard {
    fn install(self, shell: Shell) -> Shell {
        let service = PresenceService::new(
            CAPABILITY,
            "The desktop's on-screen keyboard",
            "the keyboard",
        );
        let presence = service.presence();
        let shell = shell.service(service);
        let seam = shell.seam();
        let board = Board::new(shell.typist());

        shell
            .on_shortcut(Shortcut::ToggleKeyboard, Request::new(CAPABILITY, TOGGLE))
            .surface(
                SurfaceDesc::new(move || Keypad::new(seam.clone(), board.clone()), view::view)
                    .layer(Layer::Overlay)
                    .anchor(Anchor::BOTTOM)
                    .size(view::WIDTH, view::HEIGHT)
                    .margin(Inset::edges(0.0, 0.0, MARGIN, 0.0))
                    .keyboard(KeyboardMode::None)
                    .animated(presence, view::EXIT_DURATION)
                    .on_leave(Keypad::leave),
            )
    }
}
