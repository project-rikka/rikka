use std::collections::HashMap;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Physical, Rect};
use rikka_renderer::{Audience, Scene, Stroke};
use rikka_types::{Color, CornerRadii, RoundedRect};

use super::depth::Depth;
use super::edge::{Edge, EdgeNode};
use super::view::View;

const WIDTH: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Outline {
    pub(crate) window: WindowId,
    pub(crate) colour: Color,
}

#[derive(Debug, Default)]
struct Ring {
    edge: EdgeNode,
    drawn: bool,
}

#[derive(Debug, Default)]
pub(super) struct OutlineNodes {
    rings: HashMap<WindowId, Ring>,
}

impl OutlineNodes {
    pub(super) fn begin(&mut self) {
        for ring in self.rings.values_mut() {
            ring.drawn = false;
        }
    }

    pub(super) fn show(
        &mut self,
        scene: &mut Scene,
        outline: Outline,
        around: RoundedRect,
        alpha: f32,
        view: View,
        depth: &mut Depth,
    ) -> Option<(Rect<i32, Physical>, f32)> {
        let width = view.measure(WIDTH).round();
        let ring = self.rings.entry(outline.window).or_default();
        ring.edge.draw(
            scene,
            Some(Edge {
                shape: surrounding(around, width),
                stroke: Stroke::new(width, outline.colour.scale_alpha(alpha)),
                clip: None,
            }),
            Audience::Everyone,
            depth,
        );
        ring.drawn = true;
        ring.edge.bounds().map(|(rect, alpha, _)| (rect, alpha))
    }

    pub(super) fn finish(&mut self, scene: &mut Scene) {
        self.rings.retain(|_, ring| {
            if !ring.drawn {
                ring.edge.clear(scene);
            }
            ring.drawn
        });
    }
}

fn surrounding(around: RoundedRect, width: f32) -> RoundedRect {
    let grown = |radius: f32| match radius > 0.0 {
        true => radius + width,
        false => 0.0,
    };
    let radii = around.radii;
    RoundedRect::new(
        around.rect.inflate(width),
        CornerRadii::new(
            grown(radii.top_left),
            grown(radii.top_right),
            grown(radii.bottom_right),
            grown(radii.bottom_left),
        ),
    )
}
