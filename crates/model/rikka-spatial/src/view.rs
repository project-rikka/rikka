use glam::{Mat4, Vec2, Vec3, Vec4};

use crate::aabb::Aabb;
use crate::plane::{Frustum, Plane};
use crate::ray::Ray;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub origin: Vec2,
    pub size: Vec2,
}

impl Viewport {
    pub const fn new(origin: Vec2, size: Vec2) -> Self {
        Self { origin, size }
    }

    pub const fn from_size(size: Vec2) -> Self {
        Self {
            origin: Vec2::ZERO,
            size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NdcY {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NdcDepth {
    ZeroToOne,
    ReversedZeroToOne,
    MinusOneToOne,
}

impl NdcDepth {
    fn near(self) -> f32 {
        match self {
            NdcDepth::ZeroToOne => 0.0,
            NdcDepth::ReversedZeroToOne => 1.0,
            NdcDepth::MinusOneToOne => -1.0,
        }
    }

    fn far(self) -> f32 {
        match self {
            NdcDepth::ZeroToOne | NdcDepth::MinusOneToOne => 1.0,
            NdcDepth::ReversedZeroToOne => 0.0,
        }
    }

    fn range(self) -> (f32, f32) {
        match self {
            NdcDepth::ZeroToOne | NdcDepth::ReversedZeroToOne => (0.0, 1.0),
            NdcDepth::MinusOneToOne => (-1.0, 1.0),
        }
    }

    fn near_clip_distance(self, clip: Vec4) -> f32 {
        match self {
            NdcDepth::ZeroToOne => clip.z,
            NdcDepth::ReversedZeroToOne => clip.w - clip.z,
            NdcDepth::MinusOneToOne => clip.z + clip.w,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipSpace {
    pub y: NdcY,
    pub depth: NdcDepth,
}

impl ClipSpace {
    pub const VULKAN: ClipSpace = ClipSpace {
        y: NdcY::Down,
        depth: NdcDepth::ZeroToOne,
    };
    pub const VULKAN_REVERSED_Z: ClipSpace = ClipSpace {
        y: NdcY::Down,
        depth: NdcDepth::ReversedZeroToOne,
    };
    pub const OPENGL: ClipSpace = ClipSpace {
        y: NdcY::Up,
        depth: NdcDepth::MinusOneToOne,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    pub position: Vec2,
    pub depth: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenSegment {
    pub start: Vec2,
    pub end: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    view_projection: Mat4,
    inverse: Mat4,
    viewport: Viewport,
    clip_space: ClipSpace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ClippedSegment {
    pub(crate) screen: ScreenSegment,
    pub(crate) parameters: [f32; 2],
    pub(crate) w: [f32; 2],
}

impl ClippedSegment {
    pub(crate) fn segment_parameter(&self, screen_parameter: f32) -> f32 {
        let [first_w, second_w] = self.w;
        let denominator = (1.0 - screen_parameter) * second_w + screen_parameter * first_w;
        let clip_parameter = match denominator > 0.0 {
            true => screen_parameter * first_w / denominator,
            false => screen_parameter,
        };
        self.parameters[0] + (self.parameters[1] - self.parameters[0]) * clip_parameter
    }
}

impl View {
    pub fn new(view_projection: Mat4, viewport: Viewport, clip_space: ClipSpace) -> Option<Self> {
        let determinant = view_projection.determinant();
        if determinant == 0.0 || !determinant.is_finite() || viewport.size.min_element() <= 0.0 {
            return None;
        }
        let inverse = view_projection.inverse();
        inverse.is_finite().then_some(Self {
            view_projection,
            inverse,
            viewport,
            clip_space,
        })
    }

    pub fn view_projection(&self) -> Mat4 {
        self.view_projection
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn clip_space(&self) -> ClipSpace {
        self.clip_space
    }

    pub fn is_perspective(&self) -> bool {
        self.view_projection.row(3).truncate() != Vec3::ZERO
    }

    pub fn project(&self, point: Vec3) -> Option<ScreenPoint> {
        let clip = self.view_projection * point.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(ScreenPoint {
            position: self.ndc_to_screen(Vec2::new(ndc.x, ndc.y)),
            depth: ndc.z,
        })
    }

    pub fn unproject(&self, screen: Vec2, depth: f32) -> Option<Vec3> {
        let ndc = self.screen_to_ndc(screen);
        let world = self.inverse * Vec4::new(ndc.x, ndc.y, depth, 1.0);
        if world.w == 0.0 {
            return None;
        }
        let point = world.truncate() / world.w;
        point.is_finite().then_some(point)
    }

    pub fn pick_ray(&self, screen: Vec2) -> Option<Ray> {
        let depth = self.clip_space.depth;
        let near = self.unproject(screen, depth.near())?;
        let middle = self.unproject(screen, (depth.near() + depth.far()) * 0.5)?;
        Ray::new(near, middle - near)
    }

    pub fn frustum(&self) -> Frustum {
        self.ndc_frustum(Vec2::splat(-1.0), Vec2::splat(1.0))
    }

    pub fn region_frustum(&self, corner: Vec2, opposite: Vec2) -> Frustum {
        let first = self.screen_to_ndc(corner);
        let second = self.screen_to_ndc(opposite);
        self.ndc_frustum(first.min(second), first.max(second))
    }

    pub fn screen_distance_bound(&self, bounds: &Aabb, screen: Vec2) -> f32 {
        let mut low = Vec2::splat(f32::INFINITY);
        let mut high = Vec2::splat(f32::NEG_INFINITY);
        for corner in bounds.corners() {
            let Some(projected) = self.project(corner) else {
                return 0.0;
            };
            low = low.min(projected.position);
            high = high.max(projected.position);
        }
        (low - screen).max(screen - high).max(Vec2::ZERO).length()
    }

    pub fn project_segment(&self, start: Vec3, end: Vec3) -> Option<ScreenSegment> {
        self.clip_segment(start, end).map(|clipped| clipped.screen)
    }

    pub(crate) fn clip_segment(&self, start: Vec3, end: Vec3) -> Option<ClippedSegment> {
        let mut first = self.view_projection * start.extend(1.0);
        let mut second = self.view_projection * end.extend(1.0);
        let depth = self.clip_space.depth;
        let first_distance = depth.near_clip_distance(first);
        let second_distance = depth.near_clip_distance(second);
        if first_distance < 0.0 && second_distance < 0.0 {
            return None;
        }
        let mut parameters = [0.0, 1.0];
        if first_distance < 0.0 {
            let factor = first_distance / (first_distance - second_distance);
            first = first.lerp(second, factor);
            parameters[0] = factor;
        } else if second_distance < 0.0 {
            let factor = second_distance / (second_distance - first_distance);
            second = second.lerp(first, factor);
            parameters[1] = 1.0 - factor;
        }
        if first.w <= 0.0 || second.w <= 0.0 {
            return None;
        }
        let flatten = |clip: Vec4| self.ndc_to_screen(Vec2::new(clip.x, clip.y) / clip.w);
        Some(ClippedSegment {
            screen: ScreenSegment {
                start: flatten(first),
                end: flatten(second),
            },
            parameters,
            w: [first.w, second.w],
        })
    }

    fn ndc_frustum(&self, low: Vec2, high: Vec2) -> Frustum {
        let matrix = self.view_projection;
        let (x, y, z, w) = (matrix.row(0), matrix.row(1), matrix.row(2), matrix.row(3));
        let (near, far) = self.clip_space.depth.range();
        let rows = [
            x - w * low.x,
            w * high.x - x,
            y - w * low.y,
            w * high.y - y,
            z - w * near,
            w * far - z,
        ];
        Frustum::new(rows.into_iter().filter_map(Plane::from_vec4))
    }

    fn screen_to_ndc(&self, screen: Vec2) -> Vec2 {
        let relative = (screen - self.viewport.origin) / self.viewport.size;
        let x = relative.x * 2.0 - 1.0;
        let y = match self.clip_space.y {
            NdcY::Down => relative.y * 2.0 - 1.0,
            NdcY::Up => 1.0 - relative.y * 2.0,
        };
        Vec2::new(x, y)
    }

    fn ndc_to_screen(&self, ndc: Vec2) -> Vec2 {
        let y = match self.clip_space.y {
            NdcY::Down => (ndc.y + 1.0) * 0.5,
            NdcY::Up => (1.0 - ndc.y) * 0.5,
        };
        self.viewport.origin + Vec2::new((ndc.x + 1.0) * 0.5, y) * self.viewport.size
    }
}
