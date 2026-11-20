use crate::document::Version;
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::layers::{ChannelId, Layer};
use crate::pixels::{Depth, Plane};
use crate::transform::Affine;
use crate::transform::records::{LayerGeometry, Remap, mapped_mask, rebuild_channels};
use crate::transform::resample::{Edge, Resampler, Resampling};

const SNAP: f64 = 1e-6;

pub(crate) struct Pass<'a, R> {
    pub(crate) resampler: &'a mut R,
    pub(crate) forward: Affine,
    pub(crate) inverse: Affine,
    pub(crate) from: Rect,
    pub(crate) to: Rect,
    pub(crate) fill: Vec<f32>,
    pub(crate) paint: Vec<f32>,
    pub(crate) discrete: bool,
    pub(crate) depth: Depth,
}

impl<R: Resampler> Pass<'_, R> {
    fn run(
        &mut self,
        source: &Plane,
        weight: Option<&Plane>,
        (from, to): (Rect, Rect),
        edge: Edge,
    ) -> Result<Plane> {
        let request = Resampling {
            source,
            weight,
            width: to.width(),
            height: to.height(),
            map: Affine::translation(f64::from(to.left), f64::from(to.top))
                .then(self.inverse)
                .then(Affine::translation(
                    -f64::from(from.left),
                    -f64::from(from.top),
                )),
            edge,
            discrete: self.discrete,
        };
        let plane = self.resampler.resample(&request)?;
        if (plane.width(), plane.height(), plane.depth())
            != (request.width, request.height, self.depth)
        {
            return Err(Error::Planes(
                "the resampler returned a plane of the wrong size or depth",
            ));
        }
        Ok(plane)
    }

    fn edge(&self, bounds: Rect, outside: f32) -> Edge {
        if self.forward.is_axis_aligned() && bounds.encloses(&self.from) {
            Edge::Clamp
        } else {
            Edge::Constant(outside)
        }
    }

    fn target(&self, bounds: Rect) -> Result<Rect> {
        if bounds.is_empty() {
            return Ok(bounds);
        }
        let corners = [
            (bounds.left, bounds.top),
            (bounds.right, bounds.top),
            (bounds.right, bounds.bottom),
            (bounds.left, bounds.bottom),
        ]
        .map(|(x, y)| self.forward.apply(f64::from(x), f64::from(y)));
        let extreme = |pick: fn(&(f64, f64)) -> f64, low: bool| {
            let values = corners.iter().map(pick);
            if low {
                (values.fold(f64::INFINITY, f64::min) + SNAP).floor()
            } else {
                (values.fold(f64::NEG_INFINITY, f64::max) - SNAP).ceil()
            }
        };
        let edge = |value: f64| -> Result<i32> {
            if value.abs() < f64::from(i32::MAX) {
                Ok(value as i32)
            } else {
                Err(Error::Oversized)
            }
        };
        let rect = Rect::new(
            edge(extreme(|corner| corner.0, true))?,
            edge(extreme(|corner| corner.1, true))?,
            edge(extreme(|corner| corner.0, false))?,
            edge(extreme(|corner| corner.1, false))?,
        );
        Version::Psb.check_dimensions(rect.width().max(1), rect.height().max(1))?;
        Ok(rect)
    }
}

impl<R: Resampler> Remap for Pass<'_, R> {
    fn remap_composite(&mut self, index: usize, plane: Plane) -> Result<Plane> {
        let edge = self.edge(self.from, self.fill[index]);
        self.run(&plane, None, (self.from, self.to), edge)
    }

    fn remap_layer(&mut self, layer: &Layer, background: bool) -> Result<LayerGeometry> {
        let bounds = if background {
            self.to
        } else {
            self.target(layer.bounds)?
        };
        let mask = mapped_mask(layer, |bounds| self.target(bounds))?;
        let alpha = layer.decode_channel(ChannelId::Transparency, self.depth)?;
        let channels = rebuild_channels(layer, self.depth, |channel, plane| {
            let from = layer.channel_bounds(channel.id);
            let default = |color: u8| f32::from(color) / 255.0;
            let (to, outside, weight) = match channel.id {
                ChannelId::UserMask => (
                    mask.map_or(from, |mask| mask.0),
                    layer.mask.map_or(0.0, |mask| default(mask.default_color)),
                    None,
                ),
                ChannelId::RealUserMask => (
                    mask.and_then(|mask| mask.1).unwrap_or(from),
                    layer
                        .mask
                        .and_then(|mask| mask.real)
                        .map_or(0.0, |real| default(real.default_color)),
                    None,
                ),
                ChannelId::Transparency => (bounds, 0.0, None),
                ChannelId::Color(index) if background => (
                    bounds,
                    self.paint.get(usize::from(index)).copied().unwrap_or(0.0),
                    alpha.as_ref(),
                ),
                ChannelId::Color(_) | ChannelId::Other(_) => (bounds, 0.0, alpha.as_ref()),
            };
            let edge = self.edge(from, outside);
            self.run(&plane, weight, (from, to), edge)
        })?;
        Ok(LayerGeometry {
            bounds,
            mask,
            channels: Some(channels),
        })
    }
}
