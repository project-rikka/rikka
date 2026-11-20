use std::sync::Arc;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use rikka_mesh_modifiers::{
    Array, Bevel, BevelAffect, BevelFaceStrength, BevelLimitMethod, BevelMiter, BevelOffsetType,
    BevelProfileType, BevelVertexMesh, Boolean, BooleanMaterialMode, BooleanOperation,
    BooleanSolver, BoundaryShape, Context, CornerData, CornerMapping, CullFace, DataTransfer,
    Decimate, DecimateType, Delimit, Displace, DisplaceDirection, DisplaceSpace, EdgeData,
    EdgeMapping, EdgeSplit, FaceData, FaceMapping, FitType, Mirror, MixMode, Modifier,
    ModifierKind, ObjectRef, Purpose, Shrinkwrap, SmoothByAngle, Solidify, SolidifyMode,
    StackCache, Subdivision, SubdivisionType, SymmetryAxis, Texture, TextureCoordinates,
    TextureRef, TextureSample, ThicknessMode, Triangulate, UvSmooth, VertexData, VertexMapping,
    Weld, WeldMode, WrapMethod, WrapMode, apply, evaluate, evaluate_cached,
};
use rikka_polymesh::{NgonMethod, Polymesh, QuadMethod};
use rikka_test_meshes::{Rng, Sweep, check_polymesh, corpus, is_polymesh_finite, sample_fixtures};

const TARGET: ObjectRef = ObjectRef::new(7);
const TEXTURE: TextureRef = TextureRef::new(3);
const STACKS: usize = 64;
const SAMPLE_STACKS: usize = 16;
const DEPTH: usize = 4;

struct Ripple;

impl Texture for Ripple {
    fn sample(&self, coordinate: Vec3) -> TextureSample {
        let value = (coordinate.x * 7.0).sin() * (coordinate.y * 5.0).cos() * 0.5 + 0.5;
        TextureSample {
            intensity: value,
            color: Vec4::new(value, 1.0 - value, coordinate.z.fract().abs(), 1.0),
        }
    }
}

struct World {
    matrix: Mat4,
    target: Arc<Polymesh>,
    target_matrix: Mat4,
}

impl Context for World {
    fn object_matrix(&self) -> Mat4 {
        self.matrix
    }

    fn material_count(&self) -> usize {
        3
    }

    fn matrix(&self, object: ObjectRef) -> Option<Mat4> {
        (object == TARGET).then_some(self.target_matrix)
    }

    fn mesh(&self, object: ObjectRef) -> Option<Arc<Polymesh>> {
        (object == TARGET).then(|| Arc::clone(&self.target))
    }

    fn texture(&self, texture: TextureRef) -> Option<&dyn Texture> {
        (texture == TEXTURE).then_some(&Ripple as &dyn Texture)
    }
}

fn flags3(rng: &mut Rng) -> [bool; 3] {
    [rng.flip(), rng.flip(), rng.flip()]
}

fn group(rng: &mut Rng) -> String {
    rng.pick(&["", "Upper", "Left", "Missing"]).to_owned()
}

fn kind(rng: &mut Rng) -> ModifierKind {
    match rng.below(14) {
        0 => ModifierKind::Mirror(Mirror {
            use_axis: flags3(rng),
            use_bisect_axis: flags3(rng),
            use_bisect_flip_axis: flags3(rng),
            mirror_object: rng.flip().then_some(TARGET),
            use_clip: rng.flip(),
            use_mirror_merge: rng.flip(),
            merge_threshold: rng.range(0.0, 0.1),
            bisect_threshold: rng.range(0.0, 0.1),
            use_mirror_vertex_groups: rng.flip(),
            use_mirror_u: rng.flip(),
            use_mirror_v: rng.flip(),
            use_mirror_udim: rng.flip(),
            mirror_offset: Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)),
            offset: Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)),
            use_correct_order_on_merge: rng.flip(),
        }),
        1 => ModifierKind::Subdivision(Subdivision {
            subdivision_type: rng.pick(&[SubdivisionType::CatmullClark, SubdivisionType::Simple]),
            levels: rng.int(0, 2) as u8,
            render_levels: rng.int(0, 2) as u8,
            quality: rng.int(1, 4) as u8,
            use_creases: rng.flip(),
            use_custom_normals: rng.flip(),
            use_limit_surface: rng.flip(),
            uv_smooth: rng.pick(&[
                UvSmooth::None,
                UvSmooth::PreserveCorners,
                UvSmooth::PreserveCornersAndJunctions,
                UvSmooth::PreserveCornersJunctionsAndConcave,
                UvSmooth::PreserveBoundaries,
                UvSmooth::All,
            ]),
            ..Subdivision::default()
        }),
        2 => ModifierKind::Solidify(Solidify {
            solidify_mode: rng.pick(&[SolidifyMode::Simple, SolidifyMode::Complex]),
            thickness: rng.range(-0.3, 0.3),
            offset: rng.range(-1.0, 1.0),
            thickness_clamp: rng.range(0.0, 2.0),
            use_thickness_angle_clamp: rng.flip(),
            thickness_vertex_group: rng.unit(),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            use_rim: rng.flip(),
            use_rim_only: rng.flip(),
            use_even_offset: rng.flip(),
            use_quality_normals: rng.flip(),
            use_flip_normals: rng.flip(),
            use_flat_faces: rng.flip(),
            edge_crease_inner: rng.unit(),
            edge_crease_outer: rng.unit(),
            edge_crease_rim: rng.unit(),
            material_offset: rng.int(0, 2) as i16,
            material_offset_rim: rng.int(0, 2) as i16,
            nonmanifold_thickness_mode: rng.pick(&[
                ThicknessMode::Fixed,
                ThicknessMode::Even,
                ThicknessMode::Constraints,
            ]),
            nonmanifold_boundary_mode: rng.pick(&[
                BoundaryShape::None,
                BoundaryShape::Round,
                BoundaryShape::Flat,
            ]),
            nonmanifold_merge_threshold: rng.range(0.0, 0.01),
            bevel_convex: rng.range(-1.0, 1.0),
            ..Solidify::default()
        }),
        3 => ModifierKind::Array(Array {
            fit_type: rng.pick(&[FitType::FixedCount, FitType::FitLength]),
            count: rng.int(1, 4),
            fit_length: rng.range(0.0, 4.0),
            use_constant_offset: rng.flip(),
            constant_offset_displace: rng.direction(),
            use_relative_offset: rng.flip(),
            relative_offset_displace: rng.direction(),
            use_object_offset: rng.flip(),
            offset_object: rng.flip().then_some(TARGET),
            use_merge_vertices: rng.flip(),
            use_merge_vertices_cap: rng.flip(),
            merge_threshold: rng.range(0.0, 0.1),
            start_cap: rng.chance(0.2).then_some(TARGET),
            end_cap: rng.chance(0.2).then_some(TARGET),
            offset_uv: Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)),
        }),
        4 => ModifierKind::Weld(Weld {
            mode: rng.pick(&[WeldMode::All, WeldMode::Connected]),
            merge_threshold: rng.range(0.0, 0.2),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            loose_edges: rng.flip(),
        }),
        5 => ModifierKind::Triangulate(Triangulate {
            quad_method: rng.pick(&[
                QuadMethod::Beauty,
                QuadMethod::Fixed,
                QuadMethod::Alternate,
                QuadMethod::ShortEdge,
                QuadMethod::LongEdge,
            ]),
            ngon_method: rng.pick(&[NgonMethod::Beauty, NgonMethod::Clip]),
            min_vertices: rng.int(4, 6),
            keep_custom_normals: rng.flip(),
        }),
        6 => ModifierKind::EdgeSplit(EdgeSplit {
            split_angle: rng.range(0.0, std::f32::consts::PI),
            use_edge_angle: rng.flip(),
            use_edge_sharp: rng.flip(),
        }),
        7 => ModifierKind::Displace(Displace {
            texture: rng.flip().then_some(TEXTURE),
            texture_coords: rng.pick(&[
                TextureCoordinates::Local,
                TextureCoordinates::Global,
                TextureCoordinates::Object,
                TextureCoordinates::Uv,
            ]),
            texture_coords_object: rng.flip().then_some(TARGET),
            uv_layer: rng.pick(&["", "UVMap", "Second"]).to_owned(),
            direction: rng.pick(&[
                DisplaceDirection::X,
                DisplaceDirection::Y,
                DisplaceDirection::Z,
                DisplaceDirection::Normal,
                DisplaceDirection::CustomNormal,
                DisplaceDirection::RgbToXyz,
            ]),
            space: rng.pick(&[DisplaceSpace::Local, DisplaceSpace::Global]),
            strength: rng.range(-1.0, 1.0),
            mid_level: rng.unit(),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            ..Displace::default()
        }),
        8 => ModifierKind::Shrinkwrap(Shrinkwrap {
            wrap_method: rng.pick(&[
                WrapMethod::NearestSurfacePoint,
                WrapMethod::Project,
                WrapMethod::NearestVertex,
                WrapMethod::TargetProject,
            ]),
            wrap_mode: rng.pick(&[
                WrapMode::OnSurface,
                WrapMode::Inside,
                WrapMode::Outside,
                WrapMode::OutsideSurface,
                WrapMode::AboveSurface,
            ]),
            target: rng.chance(0.9).then_some(TARGET),
            auxiliary_target: rng.chance(0.3).then_some(TARGET),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            offset: rng.range(-0.2, 0.2),
            project_limit: rng.range(0.0, 2.0),
            use_project: flags3(rng),
            subsurf_levels: rng.int(0, 2) as u8,
            use_negative_direction: rng.flip(),
            use_positive_direction: rng.flip(),
            cull_face: rng.pick(&[CullFace::Off, CullFace::Front, CullFace::Back]),
            use_invert_cull: rng.flip(),
        }),
        9 => ModifierKind::DataTransfer(DataTransfer {
            object: rng.chance(0.9).then_some(TARGET),
            use_object_transform: rng.flip(),
            use_vert_data: rng.flip(),
            data_types_verts: VertexData::from_bits_truncate(rng.below(8) as u8),
            vert_mapping: rng.pick(&[
                VertexMapping::Topology,
                VertexMapping::Nearest,
                VertexMapping::EdgeNearest,
                VertexMapping::EdgeInterpolatedNearest,
                VertexMapping::FaceNearest,
                VertexMapping::FaceInterpolatedNearest,
                VertexMapping::FaceInterpolatedNormalProjected,
            ]),
            use_edge_data: rng.flip(),
            data_types_edges: EdgeData::from_bits_truncate(rng.below(16) as u8),
            edge_mapping: rng.pick(&[
                EdgeMapping::Topology,
                EdgeMapping::VertexNearest,
                EdgeMapping::Nearest,
                EdgeMapping::FaceNearest,
                EdgeMapping::EdgeInterpolatedNormalProjected,
            ]),
            use_loop_data: rng.flip(),
            data_types_loops: CornerData::from_bits_truncate(rng.below(8) as u8),
            loop_mapping: rng.pick(&[
                CornerMapping::Topology,
                CornerMapping::NearestCornerNormal,
                CornerMapping::NearestFaceNormal,
                CornerMapping::FaceNearest,
                CornerMapping::FaceInterpolatedNearest,
                CornerMapping::FaceInterpolatedNormalProjected,
            ]),
            use_poly_data: rng.flip(),
            data_types_polys: FaceData::from_bits_truncate(rng.below(2) as u8),
            poly_mapping: rng.pick(&[
                FaceMapping::Topology,
                FaceMapping::Nearest,
                FaceMapping::Normal,
                FaceMapping::FaceInterpolatedNormalProjected,
            ]),
            use_max_distance: rng.flip(),
            max_distance: rng.range(0.0, 1.0),
            ray_radius: rng.range(0.0, 0.1),
            islands_precision: rng.unit(),
            mix_mode: rng.pick(&[
                MixMode::Replace,
                MixMode::ReplaceAboveThreshold,
                MixMode::ReplaceBelowThreshold,
                MixMode::Mix,
                MixMode::Add,
                MixMode::Subtract,
                MixMode::Multiply,
            ]),
            mix_factor: rng.unit(),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            ..DataTransfer::default()
        }),
        10 => ModifierKind::SmoothByAngle(SmoothByAngle {
            angle: rng.range(0.0, std::f32::consts::PI),
            ignore_sharpness: rng.flip(),
        }),
        11 => ModifierKind::Bevel(Bevel {
            width: rng.range(0.0, 0.3),
            segments: rng.int(1, 4),
            affect: rng.pick(&[BevelAffect::Vertices, BevelAffect::Edges]),
            offset_type: rng.pick(&[
                BevelOffsetType::Offset,
                BevelOffsetType::Width,
                BevelOffsetType::Depth,
                BevelOffsetType::Percent,
                BevelOffsetType::Absolute,
            ]),
            limit_method: rng.pick(&[
                BevelLimitMethod::None,
                BevelLimitMethod::Angle,
                BevelLimitMethod::Weight,
                BevelLimitMethod::VertexGroup,
            ]),
            angle_limit: rng.range(0.0, std::f32::consts::PI),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            profile_type: rng.pick(&[BevelProfileType::Superellipse, BevelProfileType::Custom]),
            profile: rng.unit(),
            use_clamp_overlap: rng.flip(),
            loop_slide: rng.flip(),
            mark_seam: rng.flip(),
            mark_sharp: rng.flip(),
            material: rng.int(0, 3) as i16 - 1,
            harden_normals: rng.flip(),
            face_strength_mode: rng.pick(&[
                BevelFaceStrength::None,
                BevelFaceStrength::New,
                BevelFaceStrength::Affected,
                BevelFaceStrength::All,
            ]),
            miter_outer: rng.pick(&[BevelMiter::Sharp, BevelMiter::Patch, BevelMiter::Arc]),
            miter_inner: rng.pick(&[BevelMiter::Sharp, BevelMiter::Arc]),
            spread: rng.unit(),
            vmesh_method: rng.pick(&[BevelVertexMesh::Adjacent, BevelVertexMesh::Cutoff]),
            ..Bevel::default()
        }),
        12 => ModifierKind::Boolean(Boolean {
            operation: rng.pick(&[
                BooleanOperation::Intersect,
                BooleanOperation::Union,
                BooleanOperation::Difference,
            ]),
            object: rng.chance(0.9).then_some(TARGET),
            solver: rng.pick(&[
                BooleanSolver::Fast,
                BooleanSolver::Exact,
                BooleanSolver::Manifold,
            ]),
            double_threshold: rng.range(0.0, 0.01),
            use_self: rng.flip(),
            use_hole_tolerant: rng.flip(),
            material_mode: rng.pick(&[BooleanMaterialMode::Index, BooleanMaterialMode::Transfer]),
            ..Boolean::default()
        }),
        _ => ModifierKind::Decimate(Decimate {
            decimate_type: rng.pick(&[
                DecimateType::Collapse,
                DecimateType::Unsubdivide,
                DecimateType::Planar,
            ]),
            ratio: rng.unit(),
            iterations: rng.int(0, 4) as u16,
            angle_limit: rng.range(0.0, std::f32::consts::PI),
            delimit: Delimit::from_bits_truncate(rng.below(32) as u8),
            use_dissolve_boundaries: rng.flip(),
            use_collapse_triangulate: rng.flip(),
            use_symmetry: rng.flip(),
            symmetry_axis: rng.pick(&[SymmetryAxis::X, SymmetryAxis::Y, SymmetryAxis::Z]),
            vertex_group: group(rng),
            invert_vertex_group: rng.flip(),
            vertex_group_factor: rng.range(0.0, 10.0),
        }),
    }
}

fn stack(rng: &mut Rng) -> Vec<Modifier> {
    (0..rng.int(1, DEPTH as u32))
        .map(|index| {
            let mut modifier = Modifier::new(format!("Modifier {index}"), kind(rng));
            modifier.show_viewport = rng.chance(0.9);
            modifier.show_in_editmode = rng.flip();
            modifier.show_on_cage = rng.flip();
            modifier
        })
        .collect()
}

fn world(rng: &mut Rng, target: &Polymesh) -> World {
    World {
        matrix: Mat4::from_scale_rotation_translation(
            Vec3::splat(rng.range(0.5, 2.0)),
            Quat::from_axis_angle(rng.direction(), rng.range(0.0, 3.0)),
            rng.direction(),
        ),
        target: Arc::new(target.clone()),
        target_matrix: Mat4::from_rotation_translation(
            Quat::from_axis_angle(rng.direction(), rng.range(0.0, 3.0)),
            rng.direction() * 0.3,
        ),
    }
}

fn nudged(mesh: &Polymesh, rng: &mut Rng) -> Polymesh {
    let mut moved = mesh.clone();
    for position in &mut moved.positions {
        if rng.chance(0.3) {
            *position += rng.direction() * 0.01;
        }
    }
    for key in &mut moved.shape_keys.keys {
        for position in &mut key.positions {
            if rng.chance(0.3) {
                *position += rng.direction() * 0.01;
            }
        }
    }
    moved
}

fn run(name: &str, fixtures: &[rikka_test_meshes::Fixture], stacks: usize) {
    let mut sweep = Sweep::new(name);
    let purposes = [
        Purpose::Viewport,
        Purpose::Render,
        Purpose::EditMode,
        Purpose::EditCage,
    ];
    for (index, fixture) in fixtures.iter().enumerate() {
        for case in 0..stacks {
            let mut rng = Rng::new((index * 7919 + case) as u64);
            let stack = stack(&mut rng);
            let purpose = rng.pick(&purposes);
            let target = &fixtures[rng.below(fixtures.len())].mesh;
            let world = world(&mut rng, target);
            let kinds: Vec<_> = stack.iter().map(Modifier::modifier_type).collect();
            let label = format!("{kinds:?} {purpose:?} on {} case {case}", fixture.name);
            sweep.run(&label, || {
                let finite = is_polymesh_finite(&fixture.mesh) && is_polymesh_finite(target);
                let fresh = evaluate(&fixture.mesh, &stack, &world, purpose);
                check_polymesh(&fresh.mesh, finite)?;
                let mut cache = StackCache::default();
                let cold = evaluate_cached(&fixture.mesh, &stack, &world, purpose, &mut cache);
                if cold.mesh != fresh.mesh {
                    return Err("cold cached evaluation differs from fresh evaluation".to_owned());
                }
                let moved = nudged(&fixture.mesh, &mut rng);
                let warm = evaluate_cached(&moved, &stack, &world, purpose, &mut cache);
                let expected = evaluate(&moved, &stack, &world, purpose);
                if warm.mesh != expected.mesh || warm.origins != expected.origins {
                    return Err("warm cached evaluation differs from fresh evaluation".to_owned());
                }
                let mut applied = fixture.mesh.clone();
                if apply(&mut applied, &stack[0], &world).is_ok() {
                    check_polymesh(&applied, finite)?;
                }
                Ok(())
            });
        }
    }
    sweep.finish();
}

#[test]
fn modifier_stacks_survive_corpus() {
    run("modifier corpus", &corpus(), STACKS);
}

#[test]
fn modifier_stacks_survive_samples() {
    run("modifier samples", &sample_fixtures(), SAMPLE_STACKS);
}
