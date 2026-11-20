use rikka_mesh_edit::{EdgeId, EditMesh, VertId};
use rikka_polymesh::Domain;

use crate::curve_profile::CurveProfile;
use crate::input::BevelInput;
use crate::options::{
    Affect, BevelOptions, FaceStrength, InnerMiter, Intersection, OuterMiter, ProfileType,
    WidthType,
};
use crate::spacing::{
    PRO_CIRCLE_R, PRO_LINE_R, PRO_SQUARE_IN_R, PRO_SQUARE_R, Spacing, custom_fullness,
    custom_spacing, power_segments, superellipse_fullness, superellipse_spacing,
};
use crate::weights::WeightTable;

const SQUARE_PROFILE: f32 = 0.950;
const SNAP_R: f32 = 1.0e-4;

#[derive(Clone, Debug)]
pub(crate) struct Params {
    pub(crate) offset: f32,
    pub(crate) offset_type: WidthType,
    pub(crate) custom: Option<CurveProfile>,
    pub(crate) affect: Affect,
    pub(crate) affect_vertices_odd: bool,
    pub(crate) seg: usize,
    pub(crate) profile: f32,
    pub(crate) pro_super_r: f32,
    pub(crate) weights: Option<(WeightTable, WeightTable)>,
    pub(crate) vertex_group: Option<u32>,
    pub(crate) loop_slide: bool,
    pub(crate) limit_offset: bool,
    pub(crate) offset_adjust: bool,
    pub(crate) mark_seam: bool,
    pub(crate) mark_sharp: bool,
    pub(crate) harden_normals: bool,
    pub(crate) material: Option<i32>,
    pub(crate) face_strength: FaceStrength,
    pub(crate) miter_outer: OuterMiter,
    pub(crate) miter_inner: InnerMiter,
    pub(crate) vmesh_method: Intersection,
    pub(crate) spread: f32,
    pub(crate) spacing: Spacing,
    pub(crate) spacing_miter: Option<Spacing>,
    pub(crate) seg_2: usize,
    pub(crate) fullness: f32,
}

impl Params {
    pub(crate) fn new(mesh: &EditMesh, input: &BevelInput, options: &BevelOptions) -> Params {
        let seg = options.segments.max(1) as usize;
        let profile = options.profile_shape;
        let mut pro_super_r = -(2.0f32.ln()) / profile.sqrt().ln();
        if profile >= SQUARE_PROFILE {
            pro_super_r = PRO_SQUARE_R;
        } else if (pro_super_r - PRO_CIRCLE_R).abs() < SNAP_R {
            pro_super_r = PRO_CIRCLE_R;
        } else if (pro_super_r - PRO_LINE_R).abs() < SNAP_R {
            pro_super_r = PRO_LINE_R;
        } else if pro_super_r < SNAP_R {
            pro_super_r = PRO_SQUARE_IN_R;
        }
        let custom = match &options.profile_type {
            ProfileType::Superellipse => None,
            ProfileType::Custom(profile) => Some(profile.clone()),
        };
        let (miter_outer, miter_inner) = match options.intersection {
            Intersection::Cutoff => (OuterMiter::Sharp, InnerMiter::Sharp),
            Intersection::GridFill => (options.miter_outer, options.miter_inner),
        };
        let spacing = match &custom {
            Some(profile) => custom_spacing(seg, profile),
            None => superellipse_spacing(seg, pro_super_r),
        };
        let fullness = match (&custom, seg > 1) {
            (_, false) => 0.0,
            (Some(_), true) => custom_fullness(seg, &spacing.main),
            (None, true) => superellipse_fullness(seg, profile, pro_super_r),
        };
        let spacing_miter = (custom.is_some()
            && (miter_inner != InnerMiter::Sharp || miter_outer != OuterMiter::Sharp))
            .then(|| superellipse_spacing(seg, pro_super_r));
        let weights = input.weights.as_ref().map(|layers| {
            (
                WeightTable::resolve(mesh, &layers.vert, Domain::Point),
                WeightTable::resolve(mesh, &layers.edge, Domain::Edge),
            )
        });
        let percent_like = matches!(options.width_type, WidthType::Percent | WidthType::Absolute);
        Params {
            offset: options.width,
            offset_type: options.width_type,
            affect: options.affect,
            affect_vertices_odd: options.affect == Affect::Vertices && seg % 2 == 1,
            seg,
            profile,
            pro_super_r,
            weights,
            vertex_group: input.vertex_group,
            loop_slide: options.loop_slide,
            limit_offset: options.clamp_overlap,
            offset_adjust: options.affect != Affect::Vertices && !percent_like,
            mark_seam: options.mark_seam,
            mark_sharp: options.mark_sharp,
            harden_normals: options.harden_normals,
            material: options.material.map(|material| material as i32),
            face_strength: options.face_strength,
            miter_outer,
            miter_inner,
            vmesh_method: options.intersection,
            spread: options.spread,
            seg_2: match seg > 1 {
                true => power_segments(seg),
                false => 0,
            },
            spacing,
            spacing_miter,
            fullness,
            custom,
        }
    }

    pub(crate) fn is_custom(&self) -> bool {
        self.custom.is_some()
    }

    pub(crate) fn uses_percent_or_absolute(&self) -> bool {
        matches!(self.offset_type, WidthType::Percent | WidthType::Absolute)
    }

    pub(crate) fn edge_weight(&self, edge: EdgeId) -> Option<f32> {
        self.weights
            .as_ref()
            .map(|(_, edges)| edges.get(edge.index()))
    }

    pub(crate) fn vert_weight(&self, vert: VertId) -> Option<f32> {
        self.weights
            .as_ref()
            .map(|(verts, _)| verts.get(vert.index()))
    }
}
