use rikka_compositor_geometry::{Logical, Point, Rect as WaylandRect, Transform};
use rikka_compositor_shell::{PopupId, PopupParent, ToplevelId};
use rikka_compositor_surface::SurfaceId;
use rikka_types::{Rect, Size};

use super::state::State;
use super::views::Placement;
use crate::frame::{BufferTransform, Frame, Layer, LayerContent};
use crate::view::ViewId;

impl State {
    pub(crate) fn settle(&mut self) {
        let dirty: Vec<ViewId> = self
            .views
            .records
            .iter()
            .filter(|(_, record)| record.dirty && record.gate.is_none())
            .map(|(view, _)| *view)
            .collect();
        for view in dirty {
            self.compose_view(view);
        }
        self.wake_pending_views();
    }

    pub(crate) fn window_geometry(
        &self,
        toplevel: ToplevelId,
    ) -> Option<WaylandRect<i32, Logical>> {
        let top = self.shell.toplevel(toplevel)?;
        top.window_geometry()
            .or_else(|| self.surfaces.bounding_box(top.surface()))
    }

    pub(crate) fn popup_window_origin(&self, popup: PopupId) -> Option<Point<i32, Logical>> {
        let record = self.shell.popup(popup)?;
        let parent = match record.parent()? {
            PopupParent::Toplevel(_) => Point::zero(),
            PopupParent::Popup(parent) => self.popup_window_origin(parent)?,
            PopupParent::Layer(_) => return None,
        };
        Some(parent + record.geometry()?.loc)
    }

    pub(crate) fn roots(&self, view: ViewId) -> Vec<(SurfaceId, Point<i32, Logical>)> {
        let Some(record) = self.views.records.get(&view) else {
            return Vec::new();
        };
        let Some(toplevel) = record.toplevel else {
            return Vec::new();
        };
        let Some(top) = self.shell.toplevel(toplevel) else {
            return Vec::new();
        };
        let geometry = self.window_geometry(toplevel).unwrap_or_default();
        let mut roots = vec![(top.surface(), Point::zero() - geometry.loc)];
        for popup in &record.popups {
            let Some(state) = self.shell.popup(*popup).filter(|state| state.mapped()) else {
                continue;
            };
            let Some(window) = self.popup_window_origin(*popup) else {
                continue;
            };
            let inset = state
                .window_geometry()
                .map_or(Point::zero(), |geometry| geometry.loc);
            roots.push((state.surface(), window - inset));
        }
        roots
    }

    fn compose_view(&mut self, view: ViewId) {
        let roots = self.roots(view);
        let size = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.toplevel)
            .and_then(|toplevel| self.window_geometry(toplevel))
            .map_or(Size::ZERO, |geometry| geometry.size.into());
        let mut placed = Vec::new();
        for (root, origin) in &roots {
            placed.extend(self.surfaces.tree(*root).map(|entry| Placement {
                surface: entry.surface,
                origin: *origin + entry.location,
            }));
        }
        let mut layers = Vec::with_capacity(placed.len());
        for placement in &placed {
            if let Some(layer) = self.layer(placement.surface, placement.origin) {
                layers.push(layer);
            }
        }
        let Some(record) = self.record(view) else {
            return;
        };
        record.dirty = false;
        record.placed = placed;
        let shown = record.shared.frame();
        if shown.size != size || !same_shape(&shown.layers, &layers) {
            record.generation += 1;
            record.shared.show(Frame {
                size,
                layers,
                generation: record.generation,
            });
        }
        self.surfaces_follow(view);
    }

    pub(crate) fn layer(
        &mut self,
        surface: SurfaceId,
        origin: Point<i32, Logical>,
    ) -> Option<Layer> {
        let content = self.content_of(surface)?;
        let id = self.contents.layer_id(surface);
        let node = self.surfaces.get(surface)?;
        let size = node.size()?;
        let buffer = node.buffer_size()?;
        let crop = node
            .source_in_buffer()
            .filter(|source| {
                source.loc.x != 0.0
                    || source.loc.y != 0.0
                    || source.size.w != f64::from(buffer.w)
                    || source.size.h != f64::from(buffer.h)
            })
            .map(Rect::from);
        let opaque = node.opaque().iter().map(Rect::from).collect();
        Some(Layer {
            id,
            rect: Rect::new(
                origin.x as f32,
                origin.y as f32,
                size.w as f32,
                size.h as f32,
            ),
            buffer: buffer.into(),
            crop,
            scale: node.scale() as f32,
            transform: buffer_transform(node.transform()),
            opaque,
            alpha: node.alpha() as f32,
            content,
        })
    }

    pub(crate) fn surfaces_follow(&mut self, view: ViewId) {
        let Some(record) = self.views.records.get(&view) else {
            return;
        };
        let scale = record.desired.scale;
        let key = record.pacing;
        let roots = self.roots(view);
        let mut members = Vec::new();
        for (root, _) in &roots {
            members.extend(self.surfaces.tree(*root).map(|entry| entry.surface));
            self.surfaces.set_main_output(*root, Some(key));
        }
        if let Some((root, _)) = roots.first() {
            members.push(*root);
        }
        for surface in members {
            self.surfaces.set_preferred_scale(surface, scale);
            self.enter_output(surface);
        }
    }

    pub(crate) fn unattached_follow(&mut self, toplevel: ToplevelId) {
        let Some(surface) = self.shell.toplevel(toplevel).map(|top| top.surface()) else {
            return;
        };
        self.surfaces.set_main_output(surface, Some(self.output));
        self.enter_output(surface);
    }

    pub(crate) fn enter_output(&mut self, surface: SurfaceId) {
        let Some(client) = self.surfaces.get(surface).map(|node| node.client()) else {
            return;
        };
        let bound: Vec<_> = self.outputs.resources(self.output, client).collect();
        for output in bound {
            self.surfaces.enter(surface, &output);
        }
    }
}

fn same_shape(shown: &[Layer], next: &[Layer]) -> bool {
    shown.len() == next.len()
        && shown.iter().zip(next).all(|(shown, next)| {
            shown.id == next.id
                && shown.rect == next.rect
                && shown.buffer == next.buffer
                && shown.crop == next.crop
                && shown.scale == next.scale
                && shown.transform == next.transform
                && shown.opaque == next.opaque
                && shown.alpha == next.alpha
                && same_kind(&shown.content, &next.content)
        })
}

fn same_kind(shown: &LayerContent, next: &LayerContent) -> bool {
    match (shown, next) {
        (LayerContent::Imported(_), LayerContent::Imported(_))
        | (LayerContent::Feed(_), LayerContent::Feed(_)) => true,
        (LayerContent::Solid(shown), LayerContent::Solid(next)) => shown == next,
        _ => false,
    }
}

fn buffer_transform(transform: Transform) -> BufferTransform {
    BufferTransform {
        quarter_turns: transform.quarter_turns() as u8,
        flipped: transform.is_flipped(),
    }
}
