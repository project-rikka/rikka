use std::sync::Arc;

use rikka_compositor_surface::Surfaces;
use rikka_gpu::Pixels;
use rikka_renderer::{Audience, Node, NodeId, Renderer, TextureId, Viewer};
use rikka_types::Rect as SceneRect;

use super::composition::Composition;
use super::depth::Depth;
use super::textures::ClientTextures;
use super::tree::{Placement, SurfaceTree};
use super::view::View;
use crate::cursor::{Cursor, CursorImage};

const POINTER_Z: i32 = i32::MAX;

pub(super) const CURSOR_Z: i32 = i32::MAX - 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CursorFor {
    Screen { plane: bool },
    Capture,
}

impl CursorFor {
    fn draws_image(self) -> bool {
        match self {
            CursorFor::Screen { plane } => !plane,
            CursorFor::Capture => true,
        }
    }

    fn audience(self) -> Audience {
        match self {
            CursorFor::Screen { .. } => Audience::Only(Viewer::Screen),
            CursorFor::Capture => Audience::Everyone,
        }
    }
}

pub(super) struct Pointer {
    node: NodeId,
    texture: TextureId,
    image: Arc<CursorImage>,
    at: SceneRect,
    shown: bool,
    audience: Audience,
}

impl Composition {
    pub(super) fn draw_cursor(
        &mut self,
        view: Option<View>,
        cursor: &mut Cursor,
        surfaces: &mut Surfaces,
        textures: &mut ClientTextures,
        renderer: &mut Renderer,
        drawn_for: CursorFor,
    ) {
        if cursor.hidden() {
            return self.hide_cursor(renderer);
        }
        let Some(view) = view else {
            return self.hide_cursor(renderer);
        };
        let Some((surface, hotspot)) = cursor.surface() else {
            self.forget_cursor_tree();
            return match drawn_for.draws_image() {
                true => self.draw_pointer(view, cursor, renderer, drawn_for.audience()),
                false => self.hide_pointer(renderer),
            };
        };
        self.hide_pointer(renderer);
        if !view.area.to_f64().contains(cursor.position()) {
            return self.forget_cursor_tree();
        }
        let mut tree = match self.cursor.take() {
            Some(tree) if tree.root() == surface => tree,
            Some(mut other) => {
                other.clear(&mut self.scene);
                SurfaceTree::new(surface)
            }
            None => SurfaceTree::new(surface),
        };
        tree.set_audience(drawn_for.audience());
        tree.refresh(surfaces, textures, renderer);
        let mut depth = Depth::above(CURSOR_Z);
        tree.draw(
            &mut self.scene,
            surfaces,
            textures,
            Placement {
                origin: cursor.position() - view.area.loc.to_f64() - hotspot.to_f64(),
                scale: view.scale.into(),
                alpha: 1.0,
                clip: None,
            },
            &mut depth,
        );
        self.cursor = Some(tree);
    }

    pub(super) fn hide_cursor(&mut self, renderer: &mut Renderer) {
        self.hide_pointer(renderer);
        self.forget_cursor_tree();
    }

    fn forget_cursor_tree(&mut self) {
        if let Some(mut tree) = self.cursor.take() {
            tree.clear(&mut self.scene);
        }
    }

    fn draw_pointer(
        &mut self,
        view: View,
        cursor: &mut Cursor,
        renderer: &mut Renderer,
        audience: Audience,
    ) {
        let scale = view.scale;
        let image = cursor.image(scale);
        let local = cursor.position() - view.area.loc.to_f64();
        let at = SceneRect::new(
            ((local.x * scale).round() as i32 - image.hotspot.0) as f32,
            ((local.y * scale).round() as i32 - image.hotspot.1) as f32,
            image.width as f32,
            image.height as f32,
        );
        let shown = view.area.to_f64().contains(cursor.position());
        self.place_pointer(image, at, shown, audience, renderer);
    }

    pub(super) fn place_pointer(
        &mut self,
        image: Arc<CursorImage>,
        at: SceneRect,
        shown: bool,
        audience: Audience,
        renderer: &mut Renderer,
    ) {
        if self
            .pointer
            .as_ref()
            .is_some_and(|pointer| !Arc::ptr_eq(&pointer.image, &image))
        {
            self.hide_pointer(renderer);
        }
        match &mut self.pointer {
            Some(pointer) => {
                if pointer.at != at {
                    pointer.at = at;
                    self.scene.edit(pointer.node).rect(at);
                }
                if pointer.shown != shown {
                    pointer.shown = shown;
                    self.scene.edit(pointer.node).visible(shown);
                }
                if pointer.audience != audience {
                    pointer.audience = audience;
                    self.scene.edit(pointer.node).audience(audience);
                }
            }
            None => {
                let texture = match renderer.textures().create(Pixels::premultiplied(
                    image.width,
                    image.height,
                    image.rgba(),
                )) {
                    Ok(texture) => texture,
                    Err(error) => {
                        return tracing::warn!("cannot upload cursor: {error}");
                    }
                };
                let node = self.scene.insert(
                    Node::image(at, texture)
                        .z(POINTER_Z)
                        .visible(shown)
                        .audience(audience),
                );
                self.pointer = Some(Pointer {
                    node,
                    texture,
                    image,
                    at,
                    shown,
                    audience,
                });
            }
        }
    }

    pub(super) fn hide_pointer(&mut self, renderer: &mut Renderer) {
        let Some(pointer) = self.pointer.take() else {
            return;
        };
        self.scene.remove(pointer.node);
        renderer.textures().destroy(pointer.texture);
    }
}
