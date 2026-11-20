use rikka_color_formats::Rgb;
use rikka_seam::Request;
use rikka_settings::session::color_picker::Button;
use rikka_ui::State;
use shell_kit::Placement;

use crate::CAPABILITY;
use crate::picker::frame::Frame;
use crate::picker::stage::{Stage, stage};
use crate::service::{CANCEL, CHOSE, Chosen};
use crate::shared::Shared;

#[derive(State)]
pub(crate) struct Sheet {
    shared: Shared,
    frame: Option<Frame>,
}

impl Sheet {
    #[must_use]
    pub(crate) fn new(shared: Shared, place: &Placement) -> Sheet {
        let frame = shared.frame(&place.name);
        Sheet { shared, frame }
    }

    pub(crate) fn chose(&mut self, color: Rgb, button: Button) {
        self.shared
            .seam
            .notify(Request::new(CAPABILITY, CHOSE).encode(&Chosen { color, button }));
    }

    pub(crate) fn cancelled(&mut self) {
        self.shared.seam.notify(Request::new(CAPABILITY, CANCEL));
    }
}

pub(crate) fn view(sheet: &Sheet) -> Stage {
    let preferences = &sheet.shared.preferences;
    stage(
        sheet.frame.clone(),
        preferences.copied(),
        preferences.shows_name(),
    )
}

impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet")
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}
