use rikka_present::Presenter;
use std::time::Duration;

use rikka_renderer::{
    Acquired, FrameStart, Present as _, Recorded, Renderer, Scene, SurfaceConfig, Uploads,
};
use rikka_types::Color;
use rikka_ui::{DragIcon, Rendering, State, Ui};
use rikka_ui_host::Resources;
use rikka_wayland::Host;
use rikka_wayland_client::wl_surface::WlSurface;

use crate::error::Result;
use crate::wayland::drag::pixels;

pub(crate) struct Ghost {
    ui: Ui,
    blank: Ui,
    shown: bool,
    scene: Scene,
    presenter: Presenter,
    resources: Resources,
    uploads: Uploads,
}

impl Ghost {
    pub(crate) fn new(
        renderer: &mut Renderer,
        host: &Host,
        surface: &WlSurface,
        icon: DragIcon,
        scale: f32,
    ) -> Result<Self> {
        let size = icon.size;
        let mut blank = Ui::new(
            (),
            |_: &()| rikka_ui::Element::empty(),
            rikka_theme::Theme::default(),
        );
        blank.set_background(Some(Color::TRANSPARENT));
        blank.set_viewport(size, scale);
        let mut ui = carried(icon);
        ui.set_viewport(size, scale);
        let config = SurfaceConfig {
            transparent: true,
            ..SurfaceConfig::default()
        };
        let mut presenter =
            Presenter::new(host, surface, renderer, config).map_err(rikka_renderer::Error::from)?;
        presenter.resize(
            (pixels(size.width, scale), pixels(size.height, scale)),
            f64::from(scale),
            false,
        );
        Ok(Self {
            ui,
            blank,
            shown: true,
            scene: Scene::new(),
            presenter,
            resources: Resources::new(),
            uploads: renderer.uploads(),
        })
    }

    pub(crate) fn hide(&mut self) {
        if !self.shown {
            return;
        }
        self.shown = false;
        self.blank.request_frame();
    }

    pub(crate) fn settle_pictures(&mut self) {
        self.resources.shown(self.ui.shown_images());
        if !self.resources.wants_commit() {
            return;
        }
        let frame = self.resources.commit(&self.uploads);
        self.ui
            .images_arrived(&frame.settled, !frame.updated.is_empty() || frame.repainted);
        for texture in frame.updated {
            self.scene.notify_texture_updated(texture);
        }
    }

    pub(crate) fn loads_pictures(&self) -> bool {
        self.resources.wants_commit()
    }

    pub(crate) fn wants_draw(&self) -> bool {
        let ui = match self.shown {
            true => &self.ui,
            false => &self.blank,
        };
        ui.needs_render() || self.scene.has_damage()
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer) {
        let ui = match self.shown {
            true => &mut self.ui,
            false => &mut self.blank,
        };
        ui.render(Rendering::new(&mut self.scene).resolving(&mut self.resources));
        let recorded = match record(renderer, &mut self.scene, &mut self.presenter) {
            Ok(recorded) => recorded,
            Err(error) => {
                tracing::warn!("failed to paint drag icon: {error}");
                return;
            }
        };
        if let Err(error) = self.presenter.present(recorded) {
            tracing::warn!("failed to present drag icon: {error}");
        }
    }
}

fn record(
    renderer: &mut Renderer,
    scene: &mut Scene,
    presenter: &mut Presenter,
) -> rikka_renderer::Result<Recorded> {
    match presenter.start(renderer, scene)? {
        FrameStart::Idle => return Ok(Recorded::default()),
        FrameStart::Deferred => return Ok(Recorded::deferred()),
        FrameStart::Draw => {}
    }
    match presenter.acquire_image(Duration::ZERO)? {
        Acquired::Image(image) => presenter.record_image(renderer, scene, image),
        Acquired::Stale | Acquired::NotReady => Ok(Recorded::default()),
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        self.resources.release(&self.uploads);
    }
}

fn carried(icon: DragIcon) -> Ui {
    let theme = icon.theme().clone();
    let mut ui = Ui::new(
        OutgoingDrag { icon },
        |carried: &OutgoingDrag| carried.icon.draw(),
        theme,
    );
    ui.set_background(Some(Color::TRANSPARENT));
    ui
}

#[derive(State)]
struct OutgoingDrag {
    icon: DragIcon,
}
