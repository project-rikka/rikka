use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};
use rustc_hash::FxHashMap;

use crate::camera::sphere_in_frustum;
use crate::drape::{Drape, drape_address};
use crate::draw::{Draw, Frame, OutlineSource, SkinRef};
use crate::editor::OutlinePalette;
use crate::error::{Error, Resource, Result};
use crate::fluid::wet_address;
use crate::gpu::{
    GpuBucket, GpuInstance, GpuLight, GpuShadowSlot, NONE_INDEX, element_address, instance_flags,
    light_flags,
};
use crate::light::{Light, LightKind, LightProfilePool, Placed};
use crate::look::resolve::Resolved;
use crate::look::{Characters, Look};
use crate::material::pool::Drawn;
use crate::material::{MaterialDesc, MaterialPass, MaterialPool, MaterialState, Polygon, Program};
use crate::mesh::{IndexWidth, MeshEntry, MeshId};
use crate::pool::MeshPool;
use crate::reach::{Part, Pose, Sphere, largest_stretch};
use crate::shadow::{Bounds, Plan};
use crate::vertex::{GpuAttribute, GpuMorphDelta, GpuPosition, GpuSkin};
use rikka_gpu::error::Store;

const IDENTITY_LIMIT: u32 = 0x7fff_fffe;

#[derive(Clone)]
pub(crate) struct Bucket {
    pub state: MaterialState,
    pub program: Arc<Program>,
    pub hull: bool,
    pub pass: MaterialPass,
    pub indices: IndexWidth,
    pub scatters: bool,
    pub refracts: bool,
    pub features: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct DeformKey {
    mesh: MeshId,
    skin: Option<SkinRef>,
    morph_weights: Option<(u32, u32)>,
    drape: Option<(Drape, [u32; 16])>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DrapeJob {
    pub address: u64,
    pub to_local: Mat4,
    pub linear: Mat3,
}

impl DrapeJob {
    fn new(address: u64, transform: Mat4) -> Self {
        Self {
            address,
            to_local: inverse_or_identity(transform),
            linear: Mat3::from_mat4(transform),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkinJob {
    pub positions_in: u64,
    pub attributes_in: u64,
    pub skins_in: u64,
    pub morph_starts: u64,
    pub morph_deltas: u64,
    pub out_offset: u32,
    pub vertex_count: u32,
    pub bone_offset: u32,
    pub morph_count: u32,
    pub morph_weight_first: u32,
    pub has_skeleton: bool,
    pub drape: Option<DrapeJob>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectDraw {
    pub bucket: u32,
    pub instance: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: i32,
    pub depth: f32,
    pub order: i32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeOutline {
    pub instance: u32,
    pub edges: u64,
    pub count: u32,
}

#[derive(Default)]
pub(crate) struct BuiltFrame {
    pub instances: Vec<GpuInstance>,
    pub buckets: Vec<Bucket>,
    pub skin_jobs: Vec<SkinJob>,
    deformed_by: FxHashMap<DeformKey, u32>,
    deformed_instances: Vec<u32>,
    pub deform_vertices: u32,
    pub transparent: Vec<DirectDraw>,
    pub hair: Vec<DirectDraw>,
    pub face: Vec<DirectDraw>,
    pub see_through: Vec<DirectDraw>,
    pub overlay: Vec<DirectDraw>,
    pub occluders: Vec<DirectDraw>,
    pub outlined: Vec<DirectDraw>,
    pub outline_edges: Vec<EdgeOutline>,
    pub outline_palette: OutlinePalette,
    bucket_draws: Vec<u32>,
    pub shadow_classes: Vec<ShadowClass>,
    pub shadow_programs: Vec<Arc<Program>>,
    shadow_draws: Vec<u32>,
    pub bucket_table: Vec<GpuBucket>,
    pub layout: BucketLayout,
    bucket_of: FxHashMap<BucketKey, u32>,
    pub lights: Vec<GpuLight>,
    pub shadows: Vec<GpuShadowSlot>,
    pub bones: Vec<[f32; 16]>,
    pub figure_bounds: Vec<Bounds>,
    pub tinted_casters: bool,
}

impl BuiltFrame {
    fn restart(&mut self) {
        self.instances.clear();
        self.buckets.clear();
        self.skin_jobs.clear();
        self.deformed_by.clear();
        self.deformed_instances.clear();
        self.deform_vertices = 0;
        self.transparent.clear();
        self.hair.clear();
        self.face.clear();
        self.see_through.clear();
        self.overlay.clear();
        self.occluders.clear();
        self.outlined.clear();
        self.outline_edges.clear();
        self.outline_palette.clear();
        self.bucket_draws.clear();
        self.shadow_classes.clear();
        self.shadow_classes.extend(ShadowClass::OPAQUE);
        self.shadow_programs.clear();
        self.shadow_draws.clear();
        self.shadow_draws.extend([0; ShadowClass::OPAQUE.len()]);
        self.bucket_table.clear();
        self.layout = BucketLayout::default();
        self.bucket_of.clear();
        self.lights.clear();
        self.shadows.clear();
        self.bones.clear();
        self.figure_bounds.clear();
        self.tinted_casters = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShadowClass {
    pub program: Option<u32>,
    pub masked: bool,
    pub tint: bool,
    pub indices: IndexWidth,
}

impl ShadowClass {
    const OPAQUE: [Self; 2] = [
        Self {
            program: None,
            masked: false,
            tint: false,
            indices: IndexWidth::U32,
        },
        Self {
            program: None,
            masked: false,
            tint: false,
            indices: IndexWidth::U16,
        },
    ];
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BucketLayout {
    pub shadow_base: u32,
    pub shadow_views: u32,
    pub shadow_classes: u32,
    pub late_base: Option<u32>,
}

impl BucketLayout {
    pub(crate) fn shadow(self, view: u32, class: u32) -> u32 {
        self.shadow_base + view * self.shadow_classes + class
    }

    pub(crate) fn late(self, bucket: u32) -> Option<u32> {
        self.late_base.map(|base| base + bucket)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DeformAddresses {
    pub positions: u64,
    pub attributes: u64,
}

pub(crate) fn build(
    built: &mut BuiltFrame,
    frame: &Frame,
    look: Option<&Look>,
    meshes: &MeshPool,
    materials: &MaterialPool,
) -> Result<()> {
    let camera_position = frame.camera.position();
    let outlining = frame.draws.iter().any(|draw| draw.outline.is_some());
    let occluding = frame.overlays.occluded() || outlining || frame.editor.grid.is_some();
    let frustum = frame.camera.frustum_planes();
    built.restart();

    for (number, draw) in frame.draws.iter().enumerate() {
        let (mesh, part, drawn) = resolve_draw(number, draw, frame, meshes, materials)?;
        let desc = drawn.desc;
        let bucket = bucket_for(built, &drawn, mesh);
        let shadow_class = shadow_class_for(built, desc, mesh.index_width);
        let drape = draw
            .drape
            .map(|drape| drape_address(&frame.drapes, drape, mesh.vertex_count()))
            .transpose()?;
        let deforms = mesh.deforms() || drape.is_some();
        let (positions, attributes, vertex_offset) = match deforms {
            false => (
                meshes.positions_address(),
                meshes.attributes_address(),
                mesh.vertices.offset,
            ),
            true => {
                built.deformed_instances.push(built.instances.len() as u32);
                let out_offset = deform_job(built, draw, drape, mesh, meshes)?;
                (0, 0, out_offset)
            }
        };
        let wet = match draw.wet {
            Some(wetness) => wet_address(&frame.wets, wetness, mesh.vertex_count())?,
            None => 0,
        };
        let (center, radius) = world_bounds(draw, mesh, part, frame, materials.reach(drawn.record));
        let depth = (center - camera_position).length();
        let flags = instance_flags_of(draw, desc.state, mesh.index_width)
            | facial_flag(draw, frame, look)
            | match deforms {
                true => instance_flags::DEFORMED,
                false => 0,
            };
        let instance = GpuInstance {
            positions,
            attributes,
            transform: draw.transform.to_cols_array(),
            normal_matrix: normal_matrix(draw.transform),
            center: center.to_array(),
            radius,
            material: drawn.record,
            vertex_offset,
            first_index: mesh.indices.offset + part.submesh.first_index,
            index_count: part.submesh.index_count,
            object_id: draw.object_id,
            flags,
            bucket,
            sort_key: depth.to_bits(),
            figure: draw.figure.map_or(0, |figure| figure.0),
            shadow_class,
            layers: draw.layers,
            wet,
            outline_normals: match mesh.outline_normals.count {
                0 => 0,
                _ => element_address::<u32>(
                    meshes.outline_normals.address(),
                    mesh.outline_normals.offset,
                ),
            },
            light_channels: draw.light_channels,
            outline: match draw.outline {
                Some(color) => built.outline_palette.code(color)?,
                None => 0,
            },
        };
        let index = push_instance(built, instance);
        if let Some(figure) = draw.figure {
            let figure = figure.0 as usize;
            if built.figure_bounds.len() <= figure {
                built.figure_bounds.resize(figure + 1, Bounds::EMPTY);
            }
            if radius.is_finite() && instance.index_count > 0 {
                built.figure_bounds[figure].enclose(center, radius);
            }
        }
        let visible = instance.index_count > 0 && sphere_in_frustum(&frustum, center, radius);
        if visible {
            let fills = !desc.hull
                && desc.state.pass != MaterialPass::Overlay
                && desc.state.polygon == Polygon::Fill;
            let direct = DirectDraw {
                bucket,
                instance: index,
                first_index: instance.first_index,
                index_count: instance.index_count,
                vertex_offset: instance.vertex_offset as i32,
                depth,
                order: draw.order,
            };
            if instance.outline != 0 {
                match draw.outline_source {
                    OutlineSource::Surface if fills => built.outlined.push(direct),
                    OutlineSource::Surface => {}
                    OutlineSource::Edges => {
                        let Some(Some(edges)) = mesh.edges.get(draw.submesh as usize) else {
                            return Err(Error::Invalid(format!(
                                "draw {number} outlines edges of submesh {} that has none",
                                draw.submesh
                            )));
                        };
                        if edges.count > 0 {
                            built.outline_edges.push(EdgeOutline {
                                instance: index,
                                edges: element_address::<[u32; 4]>(
                                    meshes.edges.address(),
                                    edges.offset,
                                ),
                                count: edges.count,
                            });
                        }
                    }
                }
            }
            if draw.outline_only {
                continue;
            }
            if occluding && fills {
                built.occluders.push(direct);
            }
            queue_sorted_draw(built, desc.state.pass, &instance, index, depth, draw.order);
            if let Some(look) = look
                && !desc.hull
            {
                queue_character_draw(built, look.characters, &instance, index, depth, draw.order);
            }
        }
    }

    let back_to_front = |a: &DirectDraw, b: &DirectDraw| {
        a.order
            .cmp(&b.order)
            .then_with(|| b.depth.total_cmp(&a.depth))
    };
    built.transparent.sort_by(back_to_front);
    built.see_through.sort_by(back_to_front);
    Ok(())
}

fn queue_character_draw(
    built: &mut BuiltFrame,
    characters: Characters,
    instance: &GpuInstance,
    index: u32,
    depth: f32,
    order: i32,
) {
    let direct = DirectDraw {
        bucket: instance.bucket,
        instance: index,
        first_index: instance.first_index,
        index_count: instance.index_count,
        vertex_offset: instance.vertex_offset as i32,
        depth,
        order,
    };
    if characters.hair(instance.layers) {
        built.hair.push(direct);
    }
    if characters.face(instance.layers) {
        built.face.push(direct);
    }
    if characters.see_through(instance.layers) {
        built.see_through.push(direct);
    }
}

fn resolve_draw<'a>(
    number: usize,
    draw: &Draw,
    frame: &Frame,
    meshes: &'a MeshPool,
    materials: &'a MaterialPool,
) -> Result<(&'a MeshEntry, &'a Part, Drawn<'a>)> {
    let Some(mesh) = meshes.get(draw.mesh) else {
        return Err(Error::Unknown(Resource::Mesh(draw.mesh)));
    };
    let Some(part) = mesh.parts.get(draw.submesh as usize) else {
        return Err(Error::Invalid(format!(
            "draw references submesh {} of a mesh with {}",
            draw.submesh,
            mesh.parts.len()
        )));
    };
    let Some(drawn) = materials.drawn(number, draw.material) else {
        return Err(Error::Unknown(Resource::Material(draw.material)));
    };
    if draw.object_id > IDENTITY_LIMIT {
        return Err(Error::Invalid(format!(
            "object id {} exceeds the identity buffer limit of {IDENTITY_LIMIT}",
            draw.object_id
        )));
    }
    check_deformation(draw, mesh, frame)?;
    Ok((mesh, part, drawn))
}

fn bucket_for(built: &mut BuiltFrame, drawn: &Drawn<'_>, mesh: &MeshEntry) -> u32 {
    let desc = drawn.desc;
    let state = bucket_state(desc.state);
    let scatters =
        desc.program.scatters && !desc.hull && state.pass.in_prepass() && drawn.routing.subsurface;
    let refracts = state.pass == MaterialPass::Transparent && drawn.routing.transmission;
    let features = drawn.routing.features;
    let key = BucketKey {
        state,
        program: desc.program.generation,
        hull: desc.hull,
        indices: mesh.index_width,
        scatters,
        refracts,
        features,
    };
    if let Some(&bucket) = built.bucket_of.get(&key) {
        return bucket;
    }
    built.buckets.push(Bucket {
        state,
        program: Arc::clone(&desc.program),
        hull: desc.hull,
        pass: state.pass,
        indices: mesh.index_width,
        scatters,
        refracts,
        features,
    });
    built.bucket_draws.push(0);
    let bucket = built.buckets.len() as u32 - 1;
    built.bucket_of.insert(key, bucket);
    bucket
}

fn shadow_class_for(built: &mut BuiltFrame, desc: &MaterialDesc, indices: IndexWidth) -> u32 {
    let transparent = desc.state.pass == MaterialPass::Transparent;
    let masked = desc.state.pass == MaterialPass::Masked || (transparent && desc.state.depth_write);
    let tint = transparent && !desc.state.depth_write && desc.state.casts_shadow;
    let program = match (masked || tint || desc.program.deforms) && desc.state.casts_shadow {
        true => Some(
            match built
                .shadow_programs
                .iter()
                .position(|program| program.generation == desc.program.generation)
            {
                Some(index) => index as u32,
                None => {
                    built.shadow_programs.push(Arc::clone(&desc.program));
                    built.shadow_programs.len() as u32 - 1
                }
            },
        ),
        false => None,
    };
    built.tinted_casters |= tint;
    let class = ShadowClass {
        program,
        masked: (masked || tint) && program.is_some(),
        tint,
        indices,
    };
    match built
        .shadow_classes
        .iter()
        .position(|known| *known == class)
    {
        Some(index) => index as u32,
        None => {
            built.shadow_classes.push(class);
            built.shadow_draws.push(0);
            built.shadow_classes.len() as u32 - 1
        }
    }
}

fn push_instance(built: &mut BuiltFrame, instance: GpuInstance) -> u32 {
    let index = built.instances.len() as u32;
    built.instances.push(instance);
    if instance.index_count > 0 {
        if instance.flags & instance_flags::DIRECT == 0 {
            built.bucket_draws[instance.bucket as usize] += 1;
        }
        if instance.flags & instance_flags::CASTS_SHADOW != 0 {
            built.shadow_draws[instance.shadow_class as usize] += 1;
        }
    }
    index
}

fn queue_sorted_draw(
    built: &mut BuiltFrame,
    pass: MaterialPass,
    instance: &GpuInstance,
    index: u32,
    depth: f32,
    order: i32,
) {
    let direct = DirectDraw {
        bucket: instance.bucket,
        instance: index,
        first_index: instance.first_index,
        index_count: instance.index_count,
        vertex_offset: instance.vertex_offset as i32,
        depth,
        order,
    };
    match pass {
        MaterialPass::Transparent => built.transparent.push(direct),
        MaterialPass::Overlay => built.overlay.push(direct),
        _ => {}
    }
}

fn check_deformation(draw: &Draw, mesh: &MeshEntry, frame: &Frame) -> Result<()> {
    if let Some(skin) = draw.skin {
        let end = u64::from(skin.first) + u64::from(skin.count);
        if end > frame.bones.len() as u64 {
            return Err(Error::Invalid(format!(
                "draw skin references bone {end} of {} in this frame",
                frame.bones.len()
            )));
        }
        if mesh.has_skin() && mesh.joints > skin.count {
            return Err(Error::Invalid(format!(
                "mesh weighted to {} joints drawn with a {}-bone skin",
                mesh.joints, skin.count
            )));
        }
    }
    if let Some((first, count)) = draw.morph_weights {
        let end = u64::from(first) + u64::from(count);
        if end > frame.morph_weights.len() as u64 {
            return Err(Error::Invalid(format!(
                "draw morph weights reference weight {end} of {} in this frame",
                frame.morph_weights.len()
            )));
        }
    }
    Ok(())
}

fn deform_job(
    built: &mut BuiltFrame,
    draw: &Draw,
    drape: Option<u64>,
    mesh: &MeshEntry,
    meshes: &MeshPool,
) -> Result<u32> {
    let key = DeformKey {
        mesh: draw.mesh,
        skin: draw.skin,
        morph_weights: draw.morph_weights,
        drape: draw
            .drape
            .map(|drape| (drape, draw.transform.to_cols_array().map(f32::to_bits))),
    };
    if let Some(&out_offset) = built.deformed_by.get(&key) {
        return Ok(out_offset);
    }
    let out_offset = built.deform_vertices;
    built.deform_vertices = out_offset
        .checked_add(mesh.vertex_count())
        .ok_or_else(|| Error::full(Store::DeformedVertices))?;
    built.deformed_by.insert(key, out_offset);
    built.skin_jobs.push(SkinJob {
        positions_in: element_address::<GpuPosition>(
            meshes.positions_address(),
            mesh.vertices.offset,
        ),
        attributes_in: element_address::<GpuAttribute>(
            meshes.attributes_address(),
            mesh.vertices.offset,
        ),
        skins_in: element_address::<GpuSkin>(meshes.skins.address(), mesh.skins.offset),
        morph_starts: element_address::<u32>(
            meshes.morph_starts.address(),
            mesh.morph.starts.offset,
        ),
        morph_deltas: element_address::<GpuMorphDelta>(
            meshes.morph_deltas.address(),
            mesh.morph.deltas.offset,
        ),
        out_offset,
        vertex_count: mesh.vertex_count(),
        bone_offset: draw.skin.map_or(0, |skin| skin.first),
        morph_count: draw
            .morph_weights
            .map_or(0, |(_, count)| count.min(mesh.morph.targets)),
        morph_weight_first: draw.morph_weights.map_or(0, |(first, _)| first),
        has_skeleton: mesh.has_skin() && draw.skin.is_some(),
        drape: drape.map(|address| DrapeJob::new(address, draw.transform)),
    });
    Ok(out_offset)
}

fn bucket_state(state: MaterialState) -> MaterialState {
    MaterialState {
        casts_shadow: false,
        receives_shadow: false,
        double_sided: false,
        ..state
    }
}

pub(crate) fn apply_deform_addresses(built: &mut BuiltFrame, deform: DeformAddresses) {
    let BuiltFrame {
        deformed_instances,
        instances,
        ..
    } = built;
    for &index in deformed_instances.iter() {
        let instance = &mut instances[index as usize];
        instance.positions = deform.positions;
        instance.attributes = deform.attributes;
    }
}

pub(crate) fn describe_buckets(built: &mut BuiltFrame, shadow_views: u32, late: bool) {
    let BuiltFrame {
        bucket_draws,
        bucket_table,
        shadow_draws,
        layout,
        ..
    } = built;
    bucket_table.clear();
    let mut first = 0;
    let mut push = |capacity: u32| {
        bucket_table.push(GpuBucket { first, capacity });
        first += capacity;
    };
    bucket_draws.iter().for_each(|&capacity| push(capacity));
    let shadow_base = bucket_draws.len() as u32;
    for _ in 0..shadow_views {
        shadow_draws.iter().for_each(|&capacity| push(capacity));
    }
    let shadow_classes = shadow_draws.len() as u32;
    let late_base = late.then(|| {
        bucket_draws.iter().for_each(|&capacity| push(capacity));
        shadow_base + shadow_views * shadow_classes
    });
    *layout = BucketLayout {
        shadow_base,
        shadow_views,
        shadow_classes,
        late_base,
    };
}

pub(crate) struct LightCounts {
    pub directional: u32,
    pub listed: u32,
}

fn profile_of(light: &Light, profiles: &LightProfilePool) -> Result<u32> {
    let Some(photometry) = light.photometry else {
        return Ok(NONE_INDEX);
    };
    if !matches!(light.kind, LightKind::Point { .. } | LightKind::Spot { .. }) {
        return Err(Error::Invalid(
            "light profiles shape only point and spot lights".into(),
        ));
    }
    profiles
        .slot(photometry.profile)
        .ok_or(Error::Unknown(Resource::LightProfile(photometry.profile)))
}

pub(crate) fn write_lights(
    built: &mut BuiltFrame,
    resolved: &mut Resolved,
    plan: &Plan,
    profiles: &LightProfilePool,
) -> Result<LightCounts> {
    let Resolved {
        figures,
        lights,
        owners,
        keys,
        leading,
        ..
    } = resolved;
    let leading = *leading;
    let push = |built: &mut BuiltFrame, index: usize| -> Result<()> {
        let flags = match leading == Some(index) {
            true => light_flags::LEADING,
            false => 0,
        };
        let placed = Placed {
            figure: owners[index],
            flags,
            ..plan.lights[index]
        };
        let light = &lights[index];
        built
            .lights
            .push(light.to_gpu(placed, profile_of(light, profiles)?));
        Ok(())
    };
    for pass in [false, true] {
        for (index, light) in lights.iter().enumerate() {
            if !light.kind.is_directional() == pass {
                push(built, index)?;
            }
        }
    }
    let directional = lights
        .iter()
        .filter(|light| light.kind.is_directional())
        .count() as u32;
    let listed = built.lights.len() as u32;
    for (index, key) in keys.iter().enumerate() {
        let flags = light_flags::KEY
            | match key.turned {
                true => 0,
                false => light_flags::LEADING,
            };
        let placed = Placed {
            figure: key.figure,
            flags,
            ..plan.keys[index]
        };
        figures[key.figure as usize].key_light = built.lights.len() as u32;
        built
            .lights
            .push(key.light.to_gpu(placed, profile_of(&key.light, profiles)?));
    }
    for (figure, planned) in figures.iter_mut().zip(&plan.figures) {
        figure.shadow = planned.shadow;
        figure.flags = planned.flags;
    }
    Ok(LightCounts {
        directional,
        listed,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BucketKey {
    state: MaterialState,
    program: u64,
    hull: bool,
    indices: IndexWidth,
    scatters: bool,
    refracts: bool,
    features: u32,
}

fn instance_flags_of(draw: &Draw, state: MaterialState, indices: IndexWidth) -> u32 {
    if draw.outline_only {
        return instance_flags::DIRECT
            | match indices {
                IndexWidth::U16 => instance_flags::INDEX16,
                IndexWidth::U32 => 0,
            };
    }
    let mut flags = 0;
    if indices == IndexWidth::U16 {
        flags |= instance_flags::INDEX16;
    }
    if state.casts_shadow {
        flags |= instance_flags::CASTS_SHADOW;
    }
    if state.receives_shadow {
        flags |= instance_flags::RECEIVES_SHADOW;
    }
    if draw.visible_to_picking {
        flags |= instance_flags::PICKABLE;
    }
    if draw.selected {
        flags |= instance_flags::SELECTED;
    }
    if state.double_sided {
        flags |= instance_flags::DOUBLE_SIDED;
    }
    match state.pass {
        MaterialPass::Transparent => {
            flags |= instance_flags::TRANSPARENT | instance_flags::DIRECT;
        }
        MaterialPass::Overlay => {
            flags |= instance_flags::NO_OCCLUSION | instance_flags::DIRECT;
        }
        MaterialPass::Masked => flags |= instance_flags::MASKED,
        MaterialPass::Opaque => {}
    }
    flags
}

fn facial_flag(draw: &Draw, frame: &Frame, look: Option<&Look>) -> u32 {
    let facial = look.is_some_and(|look| look.characters.facial(draw.layers));
    let headed = draw
        .figure
        .and_then(|figure| frame.figures.get(figure.0 as usize - 1))
        .is_some_and(|figure| figure.head.is_some());
    match facial && headed {
        true => instance_flags::FACIAL,
        false => 0,
    }
}

fn world_bounds(
    draw: &Draw,
    mesh: &MeshEntry,
    part: &Part,
    frame: &Frame,
    reach: Option<f32>,
) -> (Vec3, f32) {
    let local = local_bounds(draw, mesh, part, frame);
    let center = draw.transform.transform_point3(local.center);
    let radius = local.radius * largest_stretch(draw.transform);
    (center, radius + reach.unwrap_or(0.0))
}

fn local_bounds(draw: &Draw, mesh: &MeshEntry, part: &Part, frame: &Frame) -> Sphere {
    let rest = Sphere {
        center: part.submesh.center,
        radius: part.submesh.radius,
    };
    if draw.drape.is_some() {
        return Sphere {
            radius: f32::INFINITY,
            ..rest
        };
    }
    let bones = draw.skin.filter(|_| mesh.has_skin()).map(|skin| {
        let first = skin.first as usize;
        &frame.bones[first..first + skin.count as usize]
    });
    let morph_weights = draw.morph_weights.map_or(&[][..], |(first, count)| {
        let first = first as usize;
        &frame.morph_weights[first..first + count.min(mesh.morph.targets) as usize]
    });
    part.reach.posed(
        rest,
        &Pose {
            bones,
            morph_weights,
        },
    )
}

fn normal_matrix(transform: Mat4) -> [[f32; 3]; 3] {
    let linear = Mat3::from_mat4(transform);
    let determinant = linear.determinant();
    let normal = match determinant.abs() > 1e-12 {
        true => linear.inverse().transpose(),
        false => linear,
    };
    [
        normal.x_axis.to_array(),
        normal.y_axis.to_array(),
        normal.z_axis.to_array(),
    ]
}

fn inverse_or_identity(transform: Mat4) -> Mat4 {
    match transform.determinant().abs() > 1e-12 {
        true => transform.inverse(),
        false => Mat4::IDENTITY,
    }
}
