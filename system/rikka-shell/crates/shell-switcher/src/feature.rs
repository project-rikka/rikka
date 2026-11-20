use rikka_seam::Delivery;
use rikka_session::switcher;
use shell_apps::Apps;
use shell_kit::{Destination, EXIT_DURATION, Feature, KeyboardMode, Presence, Shell, SurfaceDesc};

use crate::board::Board;
use crate::view;

#[derive(Debug)]
pub struct Switcher {
    apps: Apps,
}

impl Switcher {
    #[must_use]
    pub fn new(apps: Apps) -> Switcher {
        Switcher { apps }
    }
}

impl Feature for Switcher {
    fn install(self, shell: Shell) -> Shell {
        let presence = Presence::default();
        let destination = Destination::default();
        let seam = shell.seam();
        let apps = self.apps;
        let board = {
            let presence = presence.clone();
            move |place| Board::new(seam.clone(), apps.clone(), presence.clone(), place)
        };
        shell
            .watch(switcher::CAPABILITY, {
                let (presence, destination) = (presence.clone(), destination.clone());
                move |_, incoming: &Delivery| {
                    if let Some(Some(switch)) = incoming.event().and_then(switcher::changed) {
                        destination.aim(switch.output);
                        presence.set(true);
                    }
                }
            })
            .surface(
                SurfaceDesc::destined(destination, board, view::view)
                    .on_start(Board::start)
                    .name("switcher")
                    .overlay()
                    .keyboard(KeyboardMode::None)
                    .animated(presence, EXIT_DURATION),
            )
    }
}
