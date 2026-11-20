use rikka_ime::{Composer, Panel, Startup};
use rikka_seam::Request;
use rikka_session::surfaces;
use shell_apps::Installed;
use shell_kit::{Feature, Layer, Placement, PointerMode, Positioner, Shell, SurfaceDesc};

use crate::methods::InputMethods;
use crate::service::{CAPABILITY, ImeService, REPORT};
use crate::view;
use crate::window::Window;

const CARET_GAP: f32 = 4.0;

#[derive(Debug)]
pub struct InputMethod {
    installed: Installed,
    methods: InputMethods,
}

impl InputMethod {
    #[must_use]
    pub fn new(installed: Installed, methods: InputMethods) -> Self {
        Self { installed, methods }
    }
}

impl Feature for InputMethod {
    fn install(self, shell: Shell) -> Shell {
        compose(&self.installed);

        let seam = shell.seam();
        let reporting = seam.clone();
        let panel = match Panel::start(move || {
            reporting.notify(Request::new(CAPABILITY, REPORT));
        }) {
            Ok(panel) => panel,
            Err(error) => {
                tracing::warn!("input method candidate window unavailable: {error}");
                return shell;
            }
        };
        tracing::info!("serving input method candidate window");
        self.methods.hold(panel.clone());

        let outputs = shell.outputs();
        let positioner = Positioner::new(CARET_GAP);
        let service = ImeService::new(panel.clone(), self.methods);
        shell.service(service).surface(
            SurfaceDesc::per_output(
                {
                    let positioner = positioner.clone();
                    move |_| {
                        Window::new(
                            seam.clone(),
                            panel.clone(),
                            outputs.clone(),
                            positioner.clone(),
                        )
                    }
                },
                view::view,
            )
            .on_start(Window::start)
            .on_moved(|window: &mut Window, _: Placement| window.moved())
            .name(surfaces::INPUT_PANEL)
            .layer(Layer::Overlay)
            .size(view::MIN_WIDTH, view::MIN_WIDTH / 2.0)
            .positioned(positioner)
            .pointer(PointerMode::Ignore),
        )
    }
}

fn compose(installed: &Installed) {
    let composer = match Composer::connect() {
        Ok(composer) => composer,
        Err(error) => {
            tracing::warn!("input method unavailable: {error}");
            return;
        }
    };
    tracing::info!("input method: {:?}", composer.ensure_running());

    installed.on_change(move |_| {
        if composer.ensure_running() == Startup::Replaced {
            tracing::info!("replaced input method configuration");
        }
    });
}
