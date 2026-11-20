use rikka_renderer::{Renderer, TextureId};
use rikka_ui::OffscreenRender;
use rikka_ui::consume::{OffscreenConsumer, live_offscreens};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct OffscreenFrame {
    pub(crate) updated: Vec<TextureId>,
    pub(crate) appeared: bool,
    pub(crate) holds_renders: bool,
}

#[derive(Default)]
pub(crate) struct OffscreenStore {
    renders: FxHashMap<u64, Box<dyn OffscreenRender>>,
    retiring: Vec<Box<dyn OffscreenRender>>,
}

impl OffscreenStore {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn advance(&mut self, renderer: &mut Renderer) -> OffscreenFrame {
        let mut frame = OffscreenFrame::default();
        self.retiring.clear();
        let handles = live_offscreens();

        let alive: FxHashSet<u64> = handles.iter().map(|handle| handle.id()).collect();
        let stale: Vec<u64> = self
            .renders
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in stale {
            if let Some(render) = self.renders.remove(&id) {
                self.retiring.push(render);
            }
        }

        for handle in handles {
            let Some((width, height)) = handle.size() else {
                continue;
            };
            let render = self
                .renders
                .entry(handle.id())
                .or_insert_with(|| handle.renderer());
            let Some(texture) = render.render(renderer, width, height) else {
                continue;
            };
            frame.appeared |= handle.set_texture(texture);
            frame.updated.push(texture);
        }
        frame.holds_renders = !self.renders.is_empty() || !self.retiring.is_empty();
        frame
    }
}

impl std::fmt::Debug for OffscreenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OffscreenStore")
            .field("renders", &self.renders.len())
            .finish()
    }
}
