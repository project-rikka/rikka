use std::time::Duration;

use rikka_renderer::{
    Acquired, Buffering, FrameStart, Present, Recorded, RenderOutput, Renderer, Scene, Sizing,
    SurfaceConfig, Transform,
};
use rikka_wayland::Host;
use rikka_wayland_client::Connection;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_viewport::WpViewport;
use rikka_wayland_client::wp_viewporter::WpViewporter;

use crate::error::Result;
use crate::pool::{self, Pool};

pub struct Presenter {
    surface: WlSurface,
    connection: Connection,
    viewport: Option<WpViewport>,
    presented: Option<PresentedViewport>,
    sizing: Sizing,
    pool: Box<Pool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PresentedViewport {
    source: Option<(u32, u32)>,
    destination: (i32, i32),
    scale: i32,
}

#[derive(Debug)]
pub struct Image(pool::Acquisition);

impl Presenter {
    pub fn new(
        host: &Host,
        surface: &WlSurface,
        renderer: &mut Renderer,
        config: SurfaceConfig,
    ) -> Result<Presenter> {
        let display = host.display();
        let limit = match config.buffering {
            Buffering::Triple => 3,
            Buffering::Double => 2,
        };
        let pool = Pool::new(display, surface, renderer, limit, config.transparent)?;
        tracing::debug!(
            explicit = pool.is_explicit(),
            "presenting through exported dmabufs"
        );
        let viewport = display
            .bind_optional::<WpViewporter>(1..=1)
            .map(|viewporter| {
                let viewport = viewporter.get_viewport(surface);
                viewporter.destroy();
                viewport
            });
        Ok(Presenter {
            surface: surface.clone(),
            connection: host.connection().clone(),
            viewport,
            presented: None,
            sizing: Sizing::default(),
            pool: Box::new(pool),
        })
    }

    pub fn resize(&mut self, visible: (u32, u32), scale: f64, interactive: bool) {
        self.sizing = Sizing::new(visible, scale, interactive && self.viewport.is_some());
        self.pool.sizing = self.sizing;
    }

    pub fn set_transparent(&mut self, transparent: bool) {
        self.pool.set_transparent(transparent);
    }

    pub fn set_transform(&mut self, transform: Transform) {
        self.pool.set_transform(transform);
    }

    fn commit(&mut self, recorded: Recorded) -> rikka_renderer::Result<RenderOutput> {
        let Some(id) = recorded.image() else {
            return Ok(recorded.into_output());
        };
        let output = recorded.into_output();
        if let Err(error) = self.pool.offer(&self.surface, id, &output.damage) {
            self.pool.withdraw(id);
            return Err(error.into());
        }
        frame_viewport(
            &self.surface,
            self.viewport.as_ref(),
            &mut self.presented,
            self.sizing,
        );
        self.surface.commit();
        if let Err(error) = self.connection.flush() {
            tracing::debug!("failed to flush committed frame: {error}");
        }
        Ok(output)
    }
}

fn frame_viewport(
    surface: &WlSurface,
    viewport: Option<&WpViewport>,
    current: &mut Option<PresentedViewport>,
    sizing: Sizing,
) {
    let target = match viewport {
        Some(_) => PresentedViewport {
            source: Some(sizing.visible),
            destination: sizing.destination(),
            scale: 1,
        },
        None => PresentedViewport {
            source: None,
            destination: (0, 0),
            scale: sizing.scale.round().max(1.0) as i32,
        },
    };
    if *current == Some(target) {
        return;
    }
    if current.is_none_or(|current| current.scale != target.scale) && surface.version() >= 3 {
        surface.set_buffer_scale(target.scale);
    }
    if let (Some(viewport), Some((width, height))) = (viewport, target.source) {
        viewport.set_source(0.0, 0.0, f64::from(width), f64::from(height));
        viewport.set_destination(target.destination.0, target.destination.1);
    }
    *current = Some(target);
}

impl Present for Presenter {
    type Image = Image;

    fn start(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
    ) -> rikka_renderer::Result<FrameStart> {
        Ok(self.pool.start(renderer, scene)?)
    }

    fn acquire_image(&mut self, timeout: Duration) -> rikka_renderer::Result<Acquired<Image>> {
        Ok(match self.pool.acquire(timeout)? {
            Acquired::Image(taken) => Acquired::Image(Image(taken)),
            Acquired::Stale => Acquired::Stale,
            Acquired::NotReady => Acquired::NotReady,
        })
    }

    fn record_image(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
        image: Image,
    ) -> rikka_renderer::Result<Recorded> {
        Ok(self.pool.record(renderer, scene, image.0)?)
    }

    fn present(&mut self, recorded: Recorded) -> rikka_renderer::Result<RenderOutput> {
        self.commit(recorded)
    }

    fn trim(&mut self) {
        self.pool.trim();
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }
    }
}

impl std::fmt::Debug for Presenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presenter")
            .field("sizing", &self.sizing)
            .finish_non_exhaustive()
    }
}
