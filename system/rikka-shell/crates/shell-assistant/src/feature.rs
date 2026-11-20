use shell_kit::{Anchor, Feature, KeyboardMode, Layer, Margin, PointerMode, Shell, SurfaceDesc};

use crate::assistance::Assistance;
use crate::panel::Panel;
use crate::service::AssistantService;
use crate::view;

#[derive(Debug, Default)]
pub struct AssistantPanel;

impl AssistantPanel {
    #[must_use]
    pub fn new() -> AssistantPanel {
        AssistantPanel
    }
}

impl Feature for AssistantPanel {
    fn install(self, shell: Shell) -> Shell {
        let seam = shell.seam();
        let assistance = Assistance::open(seam.clone());
        let service = AssistantService::new(assistance.clone());
        let presence = service.presence();
        let shell = shell.service(service);

        shell.surface(
            SurfaceDesc::new(
                move || Panel::new(seam.clone(), assistance.clone()),
                view::view,
            )
            .on_start(Panel::start)
            .name("assistant")
            .layer(Layer::Top)
            .anchor(Anchor::LEFT)
            .width(view::WIDTH + view::MARGIN)
            .margin(Margin::edges(view::MARGIN, 0.0, view::MARGIN, 0.0))
            .pointer(PointerMode::Inset(Margin::edges(
                0.0,
                0.0,
                0.0,
                view::MARGIN,
            )))
            .keyboard(KeyboardMode::OnDemand)
            .animated(presence, view::EXIT_DURATION),
        )
    }
}
