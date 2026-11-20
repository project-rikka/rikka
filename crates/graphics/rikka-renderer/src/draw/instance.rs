use rikka_types::{Color, CornerRadii, Rect, RoundedRect};

use crate::scene::{Node, Scene};

pub(crate) const KIND_SHAPE: u32 = 0;
pub(crate) const KIND_GLYPH_MASK: u32 = 1;
pub(crate) const KIND_TEXTURED: u32 = 2;
pub(crate) const KIND_BACKDROP: u32 = 3;
pub(crate) const KIND_GLYPH_COLOR: u32 = 4;
pub(crate) const KIND_GLYPH_CURVES: u32 = 5;

pub(crate) const FLAG_CIRCLE: u32 = 1;
pub(crate) const FLAG_CLIP: u32 = 2;
pub(crate) const FLAG_ARC: u32 = 4;
pub(crate) const FLAG_CUT: u32 = 8;
pub(crate) const FLAG_LENS: u32 = 16;
pub(crate) const FLAG_UV_SWAP: u32 = 32;
pub(crate) const FLAG_OUTSIDE_GROUP_CLIP: u32 = 64;
pub(crate) const FLAG_PIXELATED: u32 = 128;
pub(crate) const FLAG_TURNED: u32 = 256;

const KIND_MASK: u32 = 0x7;
const FLAGS_SHIFT: u32 = 3;
const FLAGS_MASK: u32 = 0x1FFF;
const COLOR_SHIFT: u32 = 16;
const COLOR_MASK: u32 = 0xFFFF;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipSlots {
    pub clip: u32,
    pub cut: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuInstance {
    pub bounds: [f32; 4],
    pub radii: [f32; 4],
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
    pub uv: [f32; 4],
    pub turn: [f32; 2],
    pub stroke_width: f32,
    pub opacity: f32,
    pub meta: u32,
    pub tex: u32,
    pub group: u32,
    pub clip_group: u32,
    pub shown: u32,
    pub clip_index: u32,
    pub cut_index: u32,
    pub padding: u32,
}

const _: () = assert!(size_of::<GpuInstance>() == 128);

impl GpuInstance {
    pub(crate) fn new(kind: u32, bounds: Rect) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            radii: [0.0; 4],
            fill: [0.0; 4],
            stroke: [0.0; 4],
            uv: [0.0, 0.0, 1.0, 1.0],
            turn: [1.0, 0.0],
            stroke_width: 0.0,
            opacity: 1.0,
            meta: kind & KIND_MASK,
            tex: 0,
            group: 0,
            clip_group: 0,
            shown: 0,
            clip_index: 0,
            cut_index: 0,
            padding: 0,
        }
    }

    pub(crate) fn kind(&self) -> u32 {
        self.meta & KIND_MASK
    }

    pub(crate) fn set_kind(&mut self, kind: u32) {
        self.meta = (self.meta & !KIND_MASK) | (kind & KIND_MASK);
    }

    pub(crate) fn placed(mut self, node: &Node, clips: ClipSlots) -> Self {
        self.group = Scene::group_slot(node.group);
        self.clip_group = Scene::group_slot(node.clip_group);
        if !node.clipped_by_group {
            self = self.with_flags(FLAG_OUTSIDE_GROUP_CLIP);
        }
        if clips.clip != 0 {
            self.clip_index = clips.clip;
            self = self.with_flags(FLAG_CLIP);
        }
        if clips.cut != 0 {
            self.cut_index = clips.cut;
            self = self.with_flags(FLAG_CUT);
        }
        self
    }

    pub(crate) fn with_radii(mut self, radii: CornerRadii) -> Self {
        self.radii = radii.to_array();
        self
    }

    pub(crate) fn with_turn(mut self, angle: f32) -> Self {
        if angle != 0.0 {
            let (sin, cos) = angle.sin_cos();
            self.turn = [cos, sin];
            self = self.with_flags(FLAG_TURNED);
        }
        self
    }

    pub(crate) fn with_arc(mut self, start: f32, sweep: f32) -> Self {
        let sweep = sweep.clamp(-std::f32::consts::TAU, std::f32::consts::TAU);
        let (sin_mid, cos_mid) = (start + sweep / 2.0).sin_cos();
        let (sin_half, cos_half) = (sweep.abs() / 2.0).sin_cos();
        self.radii = [cos_mid, sin_mid, sin_half.max(0.0), cos_half];
        self.with_flags(FLAG_ARC)
    }

    pub(crate) fn with_curves(mut self, pixels_per_em: f32) -> Self {
        self.radii = [pixels_per_em, 0.0, 0.0, 0.0];
        self
    }

    pub(crate) fn with_fill(mut self, color: Color, opacity: f32) -> Self {
        self.fill = premultiply(color, opacity);
        self.opacity = opacity;
        self
    }

    pub(crate) fn with_stroke(mut self, width: f32, color: Color, opacity: f32) -> Self {
        self.stroke = premultiply(color, opacity);
        self.stroke_width = width.max(0.0);
        self
    }

    pub(crate) fn with_texture(mut self, tex: u32, from: [f32; 2], to: [f32; 2]) -> Self {
        self.tex = tex;
        self.uv = [from[0], from[1], to[0], to[1]];
        self
    }

    pub(crate) fn with_color(mut self, color: u32) -> Self {
        self.meta =
            (self.meta & !(COLOR_MASK << COLOR_SHIFT)) | ((color & COLOR_MASK) << COLOR_SHIFT);
        self
    }

    pub(crate) fn with_flags(mut self, flags: u32) -> Self {
        self.meta |= (flags & FLAGS_MASK) << FLAGS_SHIFT;
        self
    }
}

pub(crate) fn premultiply(color: Color, opacity: f32) -> [f32; 4] {
    let [r, g, b, a] = color.to_premultiplied();
    [r * opacity, g * opacity, b * opacity, a * opacity]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuGroupClip {
    pub clip: [f32; 4],
    pub clip_radii: [f32; 4],
}

const _: () = assert!(size_of::<GpuGroupClip>() == 32);

impl GpuGroupClip {
    pub(crate) const UNCLIPPED: Self = Self {
        clip: [0.0, 0.0, -1.0, -1.0],
        clip_radii: [0.0; 4],
    };

    pub(crate) fn from_rounded(shape: RoundedRect) -> Self {
        let rect = shape.rect;
        Self {
            clip: [rect.x(), rect.y(), rect.width(), rect.height()],
            clip_radii: shape.radii.clamp_for_size(rect.size).to_array(),
        }
    }

    pub(crate) fn resolved(scene: &Scene, slot: usize) -> Self {
        match scene.group_clip_at(slot) {
            Some(clip) => Self::from_rounded(clip),
            None => Self::UNCLIPPED,
        }
    }
}
