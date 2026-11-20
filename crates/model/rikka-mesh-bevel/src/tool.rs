use crate::options::{Affect, InnerMiter, Intersection, MAX_SEGMENTS, OuterMiter, WidthType};

pub const PIXEL_MARGIN: f32 = 5.0;
const PRECISION_FACTOR: f32 = 0.1;
const PAN_SEGMENTS_PER_PIXEL: f32 = 0.02;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragValue {
    Width,
    WidthPercent,
    ProfileShape,
    Segments,
}

impl DragValue {
    #[must_use]
    pub fn for_width_type(width_type: WidthType) -> DragValue {
        match width_type {
            WidthType::Percent => DragValue::WidthPercent,
            _ => DragValue::Width,
        }
    }

    #[must_use]
    pub fn start(self) -> f32 {
        match self {
            DragValue::Width | DragValue::WidthPercent => 0.0,
            DragValue::ProfileShape => 0.5,
            DragValue::Segments => 1.0,
        }
    }

    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        match self {
            DragValue::Width => value.clamp(0.0, 1.0e6),
            DragValue::WidthPercent => value.clamp(0.0, 100.0),
            DragValue::ProfileShape => value.clamp(0.0, 1.0),
            DragValue::Segments => value.clamp(1.0, MAX_SEGMENTS as f32),
        }
    }

    #[must_use]
    pub fn snap_increment(self, precision: bool) -> f32 {
        let (increment, factor) = match self {
            DragValue::Width => (0.1, 0.1),
            DragValue::WidthPercent => (5.0, 0.2),
            DragValue::ProfileShape | DragValue::Segments => (0.0, 0.0),
        };
        match precision {
            true => increment * factor,
            false => increment,
        }
    }

    #[must_use]
    pub fn scale(self, pixels_per_inch: f32, pixel_size: f32, max_object_scale: f32) -> f32 {
        match self {
            DragValue::Width => pixel_size / max_object_scale,
            DragValue::WidthPercent => 100.0 / pixels_per_inch,
            DragValue::ProfileShape => 1.0 / pixels_per_inch,
            DragValue::Segments => 4.0 / pixels_per_inch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueDrag {
    pub kind: DragValue,
    pub scale: f32,
    pub initial_length: f32,
}

impl ValueDrag {
    #[must_use]
    pub fn new(kind: DragValue, scale: f32, mouse_distance: f32, current: f32) -> ValueDrag {
        let start = kind.start();
        let initial_length = match current != start {
            true => (start + scale * (mouse_distance - PIXEL_MARGIN) - current) / scale,
            false => mouse_distance,
        };
        ValueDrag {
            kind,
            scale,
            initial_length,
        }
    }

    #[must_use]
    pub fn value(&self, mouse_distance: f32, precision_anchor: Option<f32>, snap: bool) -> f32 {
        let moved = (mouse_distance - PIXEL_MARGIN) - self.initial_length;
        let mut value = self.kind.start() + moved * self.scale;
        if let Some(anchor) = precision_anchor {
            value = (value - anchor) * PRECISION_FACTOR + anchor;
        }
        if snap {
            let increment = self.kind.snap_increment(precision_anchor.is_some());
            if increment != 0.0 {
                value = increment * (value / increment).round();
            }
        }
        self.kind.clamp(value)
    }
}

#[must_use]
pub fn segments_from_value(value: f32) -> u32 {
    (DragValue::Segments.clamp(value) + 0.5) as u32
}

#[must_use]
pub fn pan_segments(segments: f32, delta_pixels: f32) -> f32 {
    let delta = PAN_SEGMENTS_PER_PIXEL * delta_pixels;
    match segments >= 1.0 && segments + delta < 1.0 {
        true => 1.0,
        false => segments + delta,
    }
}

#[must_use]
pub fn segments_up(segments: f32) -> f32 {
    segments + 1.0
}

#[must_use]
pub fn segments_down(segments: f32) -> f32 {
    (segments - 1.0).max(1.0)
}

#[must_use]
pub fn next_width_type(width_type: WidthType) -> WidthType {
    match width_type {
        WidthType::Offset => WidthType::Width,
        WidthType::Width => WidthType::Depth,
        WidthType::Depth => WidthType::Percent,
        WidthType::Percent | WidthType::Absolute => WidthType::Offset,
    }
}

#[must_use]
pub fn next_outer_miter(miter: OuterMiter) -> OuterMiter {
    match miter {
        OuterMiter::Sharp => OuterMiter::Patch,
        OuterMiter::Patch => OuterMiter::Arc,
        OuterMiter::Arc => OuterMiter::Sharp,
    }
}

#[must_use]
pub fn next_inner_miter(miter: InnerMiter) -> InnerMiter {
    match miter {
        InnerMiter::Sharp => InnerMiter::Arc,
        InnerMiter::Arc => InnerMiter::Sharp,
    }
}

#[must_use]
pub fn next_affect(affect: Affect) -> Affect {
    match affect {
        Affect::Vertices => Affect::Edges,
        Affect::Edges => Affect::Vertices,
    }
}

#[must_use]
pub fn next_intersection(intersection: Intersection) -> Intersection {
    match intersection {
        Intersection::GridFill => Intersection::Cutoff,
        Intersection::Cutoff => Intersection::GridFill,
    }
}
