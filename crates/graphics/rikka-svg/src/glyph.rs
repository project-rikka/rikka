use std::borrow::Cow;

use rikka_types::Rect;
use rikka_vector::{Canvas, Transform};

use crate::document::{MAX_PIXELS, Svg};
use crate::error::{Error, Result};
use crate::render::bounds::node_bounds;
use crate::render::{Host, draw_node};
use crate::tree::{Group, Node};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphPixels {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub premultiplied: Vec<u8>,
}

impl Svg {
    pub fn render_glyph(
        &self,
        glyph: u16,
        scale: f32,
        offset_x: f32,
    ) -> Result<Option<GlyphPixels>> {
        let (node, parent) = self.glyph_node(glyph);
        let Some(bounds) = node_bounds(&node, &parent, true) else {
            return Ok(None);
        };
        let Some(area) = PixelBounds::from_bounds(bounds, scale, offset_x) else {
            return Ok(None);
        };
        let pixels = u64::from(area.width) * u64::from(area.height);
        if pixels > MAX_PIXELS {
            return Err(Error::TooManyPixels {
                width: area.width,
                height: area.height,
                pixels,
                limit: MAX_PIXELS,
            });
        }
        let mut canvas = Canvas::new();
        canvas.begin(area.width, area.height);
        let placed = parent
            .then(Transform::scale(scale, scale))
            .then(Transform::translate(
                offset_x - area.left as f32,
                -area.top as f32,
            ));
        draw_node(
            &node,
            &placed,
            Host::of(&Transform::scale(scale, scale)),
            &mut canvas,
        );
        Ok(Some(GlyphPixels {
            left: area.left,
            top: -area.top,
            width: area.width,
            height: area.height,
            premultiplied: canvas.pixels().to_vec(),
        }))
    }

    pub(crate) fn glyph_node(&self, glyph: u16) -> (Cow<'_, Node>, Transform) {
        let id = format!("glyph{glyph}");
        let root = &self.tree.root;
        match find(root, &id, &root.transform) {
            Some((node, parent)) => (Cow::Borrowed(node), parent),
            None => (Cow::Owned(Node::Group(root.clone())), Transform::IDENTITY),
        }
    }
}

fn find<'t>(group: &'t Group, id: &str, transform: &Transform) -> Option<(&'t Node, Transform)> {
    for child in &group.children {
        let own = match child {
            Node::Group(group) => &group.id,
            Node::Shape(shape) => &shape.id,
            Node::Picture(picture) => &picture.id,
            Node::Glyph(_) => continue,
        };
        if own == id {
            return Some((child, *transform));
        }
        if let Node::Group(inner) = child {
            let found = find(inner, id, &inner.transform.then(*transform));
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

struct PixelBounds {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

impl PixelBounds {
    fn from_bounds(bounds: Rect, scale: f32, offset_x: f32) -> Option<PixelBounds> {
        let left = (bounds.min_x() * scale + offset_x).floor();
        let top = (bounds.min_y() * scale).floor();
        let right = (bounds.max_x() * scale + offset_x).ceil();
        let bottom = (bounds.max_y() * scale).ceil();
        let in_range = [left, top, right, bottom]
            .iter()
            .all(|edge| edge.is_finite() && edge.abs() < i32::MAX as f32);
        if !in_range || right <= left || bottom <= top {
            return None;
        }
        Some(PixelBounds {
            left: left as i32,
            top: top as i32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
}
