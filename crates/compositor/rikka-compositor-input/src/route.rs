use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_types::OutputId;

const EDGE_INSET: f64 = 0.5;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct PointerRestrictions: u8 {
        const CONSTRAINED = 1;
        const GRABBED = 1 << 1;
        const HIDDEN = 1 << 2;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteOutput {
    pub area: Rect<i32, Logical>,
    pub scale: f64,
    pub cursor: Option<OutputId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointerRoute {
    pub outputs: Vec<RouteOutput>,
    pub restrictions: PointerRestrictions,
}

impl PointerRoute {
    pub fn is_free(&self) -> bool {
        self.restrictions.is_empty() && !self.outputs.is_empty()
    }

    pub fn output_at(&self, position: Point<f64, Logical>) -> Option<&RouteOutput> {
        self.outputs
            .iter()
            .find(|output| output.area.to_f64().contains(position))
    }

    pub fn clamp(&self, position: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.output_at(position).is_some() {
            return position;
        }
        self.outputs
            .iter()
            .map(|output| clamp_into(output.area, position))
            .min_by(|a, b| {
                squared_distance(*a, position).total_cmp(&squared_distance(*b, position))
            })
            .unwrap_or(position)
    }

    pub(crate) fn move_pointer(
        &self,
        from: Point<f64, Logical>,
        delta: Point<f64, Logical>,
    ) -> Option<(Point<f64, Logical>, &RouteOutput)> {
        if !self.is_free() {
            return None;
        }
        let to = self.clamp(from + delta);
        let output = self.output_at(to)?;
        output.cursor?;
        Some((to, output))
    }
}

fn clamp_into(area: Rect<i32, Logical>, position: Point<f64, Logical>) -> Point<f64, Logical> {
    let area = area.to_f64();
    let reach = Size::<f64, Logical>::new(
        (area.size.w - EDGE_INSET).max(0.0),
        (area.size.h - EDGE_INSET).max(0.0),
    );
    Point::new(
        position.x.clamp(area.loc.x, area.loc.x + reach.w),
        position.y.clamp(area.loc.y, area.loc.y + reach.h),
    )
}

fn squared_distance(a: Point<f64, Logical>, b: Point<f64, Logical>) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}
