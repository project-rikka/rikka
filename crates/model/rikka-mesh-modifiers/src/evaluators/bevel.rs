use rikka_mesh_bevel::{
    Affect, BevelInput, BevelOptions, CurveProfile, FaceStrength, HandleType, InnerMiter,
    Intersection, Limit, OuterMiter, ProfilePoint, ProfileType, WeightLayers, WidthType, bevel,
};
use rikka_polymesh::Polymesh;

use super::Scope;
use super::editing::Editing;
use crate::kinds::bevel::{
    Bevel, BevelAffect, BevelFaceStrength, BevelLimitMethod, BevelMiter, BevelOffsetType,
    BevelProfileType, BevelVertexMesh, CustomProfile, ProfileHandle,
};
use crate::normals;
use crate::note::SkipReason;

pub(crate) fn evaluate(
    settings: &Bevel,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
) -> Result<(), SkipReason> {
    if mesh.point_count() == 0 {
        return Ok(());
    }
    let options = options(settings, scope.ctx.material_count())?;
    options
        .validate()
        .map_err(|_| SkipReason::InvalidSettings)?;
    let mut working = mesh.clone();
    let carried = !settings.harden_normals && normals::encode_custom(&mut working);
    let mut editing = Editing::new(&working)?;
    let layers = WeightLayers {
        vert: weight_layer(&settings.vertex_weight),
        edge: weight_layer(&settings.edge_weight),
    };
    let mut input = BevelInput::from_limit(
        &editing.mesh,
        options.affect,
        limit(settings, mesh),
        &layers,
    )
    .map_err(|_| SkipReason::InvalidSettings)?;
    if !mesh.vertex_groups.has_weights() {
        input.vertex_group = None;
    }
    bevel(&mut editing.mesh, &input, &options).map_err(|_| SkipReason::InvalidMesh)?;
    let mut result = editing.finish();
    if carried {
        normals::decode_custom(&mut result);
    }
    *mesh = result;
    Ok(())
}

fn weight_layer(name: &str) -> String {
    match name.starts_with('.') {
        true => String::new(),
        false => name.to_owned(),
    }
}

fn limit(settings: &Bevel, mesh: &Polymesh) -> Limit {
    match settings.limit_method {
        BevelLimitMethod::None => Limit::None,
        BevelLimitMethod::Angle => Limit::Angle(settings.angle_limit),
        BevelLimitMethod::Weight => Limit::Weight,
        BevelLimitMethod::VertexGroup => {
            match (!settings.vertex_group.is_empty())
                .then(|| mesh.vertex_groups.index(&settings.vertex_group))
                .flatten()
            {
                Some(group) => Limit::VertexGroup {
                    group,
                    invert: settings.invert_vertex_group,
                },
                None => Limit::None,
            }
        }
    }
}

fn options(settings: &Bevel, material_count: usize) -> Result<BevelOptions, SkipReason> {
    let largest = material_count as i32 - 1;
    let material = i32::from(settings.material).clamp(-1, largest.max(-1));
    Ok(BevelOptions {
        affect: match settings.affect {
            BevelAffect::Vertices => Affect::Vertices,
            BevelAffect::Edges => Affect::Edges,
        },
        width_type: match settings.offset_type {
            BevelOffsetType::Offset => WidthType::Offset,
            BevelOffsetType::Width => WidthType::Width,
            BevelOffsetType::Depth => WidthType::Depth,
            BevelOffsetType::Percent => WidthType::Percent,
            BevelOffsetType::Absolute => WidthType::Absolute,
        },
        width: settings.width,
        segments: settings.segments,
        profile_shape: settings.profile,
        profile_type: match settings.profile_type {
            BevelProfileType::Superellipse => ProfileType::Superellipse,
            BevelProfileType::Custom => ProfileType::Custom(profile(&settings.custom_profile)?),
        },
        material: u32::try_from(material).ok(),
        harden_normals: settings.harden_normals,
        clamp_overlap: settings.use_clamp_overlap,
        loop_slide: settings.loop_slide,
        mark_seam: settings.mark_seam,
        mark_sharp: settings.mark_sharp,
        miter_outer: match settings.miter_outer {
            BevelMiter::Sharp => OuterMiter::Sharp,
            BevelMiter::Patch => OuterMiter::Patch,
            BevelMiter::Arc => OuterMiter::Arc,
        },
        miter_inner: match settings.miter_inner {
            BevelMiter::Sharp => InnerMiter::Sharp,
            BevelMiter::Patch | BevelMiter::Arc => InnerMiter::Arc,
        },
        spread: settings.spread,
        intersection: match settings.vmesh_method {
            BevelVertexMesh::Adjacent => Intersection::GridFill,
            BevelVertexMesh::Cutoff => Intersection::Cutoff,
        },
        face_strength: match settings.face_strength_mode {
            BevelFaceStrength::None => FaceStrength::None,
            BevelFaceStrength::New => FaceStrength::New,
            BevelFaceStrength::Affected => FaceStrength::Affected,
            BevelFaceStrength::All => FaceStrength::All,
        },
    })
}

fn handle(handle: ProfileHandle) -> HandleType {
    match handle {
        ProfileHandle::Auto => HandleType::Auto,
        ProfileHandle::Vector => HandleType::Vector,
        ProfileHandle::Free => HandleType::Free,
        ProfileHandle::Aligned => HandleType::Align,
    }
}

fn profile(custom: &CustomProfile) -> Result<CurveProfile, SkipReason> {
    let points = custom
        .points
        .iter()
        .map(|point| {
            ProfilePoint::new(point.location, HandleType::Auto)
                .with_handle_types(
                    handle(point.handle_left_type),
                    handle(point.handle_right_type),
                )
                .with_handles(point.handle_left, point.handle_right)
        })
        .collect();
    Ok(CurveProfile::new(points)
        .map_err(|_| SkipReason::InvalidSettings)?
        .with_sample_straight_edges(custom.use_sample_straight_edges)
        .with_sample_even_lengths(custom.use_sample_even_lengths))
}
