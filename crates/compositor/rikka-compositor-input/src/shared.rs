use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::{ArcSwap, Guard};
use rikka_compositor_geometry::{Logical, Point};

use crate::route::{PointerRoute, RouteOutput};

#[derive(Debug)]
pub struct PointerShared {
    position: AtomicU64,
    route: ArcSwap<PointerRoute>,
}

impl PointerShared {
    pub fn new(position: Point<f64, Logical>) -> Self {
        Self {
            position: AtomicU64::new(pack(position)),
            route: ArcSwap::from_pointee(PointerRoute::default()),
        }
    }

    pub fn position(&self) -> Point<f64, Logical> {
        unpack(self.position.load(Ordering::Acquire))
    }

    pub fn warp(&self, to: Point<f64, Logical>) {
        self.position.store(pack(to), Ordering::Release);
    }

    pub fn clamp(&self, position: Point<f64, Logical>) -> Point<f64, Logical> {
        self.route.load().clamp(position)
    }

    pub fn publish(&self, route: PointerRoute) {
        self.route.store(Arc::new(route));
    }

    pub(crate) fn current_route(&self) -> Guard<Arc<PointerRoute>> {
        self.route.load()
    }

    pub(crate) fn move_pointer<'route>(
        &self,
        route: &'route PointerRoute,
        delta: Point<f64, Logical>,
    ) -> Option<(Point<f64, Logical>, &'route RouteOutput)> {
        let mut landed = None;
        self.position
            .try_update(Ordering::AcqRel, Ordering::Acquire, |packed| {
                let (to, output) = route.move_pointer(unpack(packed), delta)?;
                let stored = pack(to);
                landed = Some((unpack(stored), output));
                Some(stored)
            })
            .ok()?;
        landed
    }
}

impl Default for PointerShared {
    fn default() -> Self {
        Self::new(Point::zero())
    }
}

fn pack(point: Point<f64, Logical>) -> u64 {
    (u64::from((point.x as f32).to_bits()) << 32) | u64::from((point.y as f32).to_bits())
}

fn unpack(packed: u64) -> Point<f64, Logical> {
    Point::new(
        f64::from(f32::from_bits((packed >> 32) as u32)),
        f64::from(f32::from_bits(packed as u32)),
    )
}
