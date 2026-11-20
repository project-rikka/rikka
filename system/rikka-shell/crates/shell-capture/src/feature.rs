use rikka_network::Network;
use rikka_seam::Request;
use rikka_session::shortcuts::Shortcut;
use rikka_session::toplevels;
use rikka_ui::Inset;
use shell_feeds::ScreenRecording;
use shell_kit::{Anchor, EXIT_DURATION, Feature, Layer, PointerMode, Shell, SurfaceDesc};
use shell_markup::Editing;

use crate::aim::Mode;
use crate::preferences;
use crate::selector::{self, Selector};
use crate::service::{CAPABILITY, CaptureService, PICK, PICTURE, STOP, VIDEO};
use crate::shared::Shared;
use crate::snip::Snip;
use crate::toolbar::{self, Toolbar};

#[derive(Debug)]
pub struct Capture {
    snip: Snip,
    editing: Editing,
    recording: ScreenRecording,
    network: Network,
}

impl Capture {
    #[must_use]
    pub fn new(snip: Snip, editing: Editing, network: Network) -> Capture {
        Capture {
            snip,
            editing,
            recording: ScreenRecording::new(Request::new(CAPABILITY, STOP)),
            network,
        }
    }

    #[must_use]
    pub fn recording(&self) -> ScreenRecording {
        self.recording.clone()
    }
}

impl Feature for Capture {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(crate::settings::schema());
        let (shell, preferences) = preferences::follow(shell);

        let shared = Shared::new(shell.seam(), self.recording);
        let service = CaptureService::new(
            shell.clipboard(),
            preferences,
            shared.clone(),
            self.snip,
            self.editing,
            self.network,
        );

        let shell = shell.service(service);
        let seam = shell.seam();

        shell
            .on_shortcuts(|shortcut| match shortcut {
                Shortcut::ScreenshotScreen => Some(Request::new(CAPABILITY, PICTURE)),
                Shortcut::RecordScreen => Some(Request::new(CAPABILITY, VIDEO)),
                Shortcut::ScreenshotRegion => {
                    Some(Request::new(CAPABILITY, PICK).encode(&Mode::Picture))
                }
                Shortcut::RecordRegion => Some(Request::new(CAPABILITY, PICK).encode(&Mode::Video)),
                Shortcut::ScanCode => Some(Request::new(CAPABILITY, PICK).encode(&Mode::Scan)),
                _ => None,
            })
            .watch(toplevels::CAPABILITY, {
                let windows = shared.windows.clone();
                move |_, incoming| {
                    let Some(event) = incoming.event() else {
                        return;
                    };
                    if let Some(state) = toplevels::changed(event) {
                        windows.follow(&state);
                    }
                }
            })
            .surface(
                SurfaceDesc::per_output(
                    {
                        let shared = shared.clone();
                        move |place| Selector::new(shared.clone(), place)
                    },
                    selector::view,
                )
                .on_start(Selector::start)
                .name("capture-sheet")
                .modal(shared.picking.clone(), EXIT_DURATION),
            )
            .surface(
                SurfaceDesc::new(
                    {
                        let shared = shared.clone();
                        move || {
                            Toolbar::new(
                                seam.clone(),
                                shared.aimed.clone(),
                                shared.windows.clone(),
                                shared.codes.clone(),
                            )
                        }
                    },
                    toolbar::view,
                )
                .on_start(Toolbar::start)
                .on_leave(Toolbar::leave)
                .name("capture-bar")
                .layer(Layer::Overlay)
                .anchor(Anchor::TOP)
                .size(toolbar::WIDTH, toolbar::MARGIN + toolbar::HEIGHT)
                .pointer(PointerMode::Inset(Inset::edges(
                    toolbar::MARGIN,
                    0.0,
                    0.0,
                    0.0,
                )))
                .fitted()
                .animated(shared.picking.clone(), EXIT_DURATION),
            )
    }
}
