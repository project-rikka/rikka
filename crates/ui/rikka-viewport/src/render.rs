use std::sync::Weak;

use rikka_renderer::{Renderer, TextureId};
use rikka_renderer_3d::{Frame, Pick, Three, Viewport, ViewportConfig, Warming};
use rikka_ui::OffscreenRender;

use crate::handle::{SceneCtx, Shared};

pub(crate) struct Render {
    shared: Weak<Shared>,
    entry: Option<Entry>,
    retiring: Option<Viewport>,
}

struct Entry {
    viewport: Viewport,
    size: (u32, u32),
    revision: u64,
    pick: Pick,
    picking: Option<(i32, i32)>,
    held: Option<Frame>,
}

impl Entry {
    fn answer(&mut self, renderer: &mut Renderer, shared: &Shared) {
        let Some((x, y)) = self.picking else {
            return;
        };
        if renderer.three().poll_pick(&mut self.pick) {
            self.picking = None;
            shared.answer_pick(self.pick.at(x, y));
        }
    }

    fn ask(&mut self, renderer: &mut Renderer, shared: &Shared) {
        let Some((x, y)) = shared.take_pick_request() else {
            return;
        };
        match renderer
            .three()
            .start_pick(&self.viewport, x, y, 1, 1, &mut self.pick)
        {
            Ok(()) => self.picking = Some((x, y)),
            Err(error) => {
                tracing::warn!("failed to answer 3D pick: {error}");
                self.picking = None;
                shared.answer_pick(None);
            }
        }
    }
}

impl Render {
    pub(crate) fn new(shared: Weak<Shared>) -> Self {
        Self {
            shared,
            entry: None,
            retiring: None,
        }
    }

    fn refresh(&mut self, renderer: &mut Renderer, shared: &Shared, size: (u32, u32)) {
        let revision = shared.config_revision();
        let fresh = self
            .entry
            .as_ref()
            .is_some_and(|entry| entry.size == size && entry.revision == revision);
        if fresh {
            return;
        }
        if let Some(previous) = self.entry.take() {
            if previous.picking.is_some() {
                shared.answer_pick(None);
            }
            self.retiring = Some(previous.viewport);
        }
        let config = ViewportConfig {
            width: size.0,
            height: size.1,
            warming: Warming::DrawPlain,
            ..shared.config()
        };
        match renderer.three().create_viewport(config) {
            Ok(viewport) => {
                let pick = renderer.three().create_pick();
                self.entry = Some(Entry {
                    viewport,
                    size,
                    revision,
                    pick,
                    picking: None,
                    held: None,
                });
            }
            Err(error) => tracing::error!("failed to create 3D viewport: {error}"),
        }
    }
}

impl OffscreenRender for Render {
    fn render(&mut self, renderer: &mut Renderer, width: u32, height: u32) -> Option<TextureId> {
        self.retiring = None;
        let shared = self.shared.upgrade()?;
        self.refresh(renderer, &shared, (width, height));
        let entry = self.entry.as_mut()?;
        entry.answer(renderer, &shared);

        let texture = entry.viewport.texture();
        let built = shared.build(SceneCtx {
            renderer,
            width,
            height,
        });
        let scene = built.or_else(|| entry.held.take())?;

        if let Err(error) = renderer.three().render(&mut entry.viewport, &scene) {
            tracing::error!("failed to draw 3D frame: {error}");
            return None;
        }
        entry.held = entry.viewport.is_converging().then_some(scene);
        entry.ask(renderer, &shared);
        Some(texture)
    }
}
