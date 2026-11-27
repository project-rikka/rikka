use rikka_model_core::glam::Vec3;
use rikka_model_core::{
    CoordinateSystem, Error, Filter, Image, ImportCtx, Mesh, Model, ModelBuilder, Node, NodeId,
    Result, Sampler, Skin, Submesh, Texture, TextureId, Transform, Vertices, Warning, Wrap,
};

use super::wire::PmxFile;
use crate::MMD_UNIT_SCALE;
use crate::pmx::document::{Pmx, PmxToon};
use crate::pmx::surface::{SHARED_TOONS, SurfaceTextures, material_of, shared_toon_path};

pub(super) fn build(parsed: PmxFile, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let PmxFile {
        mut pmx,
        mut vertices,
        indices,
        counts,
        targets,
    } = parsed;
    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(MMD_UNIT_SCALE)
        .generator(pmx.name.clone());
    if bind_to_bones(&mut vertices, pmx.bones.len()) {
        builder.warn(Warning::Repaired(
            "vertices reference missing bones, binding them to the first bone".to_owned(),
        ));
    }

    let pictures = load_pictures(&pmx, &mut builder, ctx)?;
    let ramps = ramps_of(&pmx, &pictures, &mut builder);
    let shared_ramps = load_shared_ramps(&pmx, &mut builder, ctx)?;
    let textures = SurfaceTextures {
        pictures: &pictures,
        names: &pmx.textures,
        ramps: &ramps,
        shared_ramps: &shared_ramps,
    };

    let mut first_index = 0;
    let mut submeshes = Vec::with_capacity(pmx.materials.len());
    for (order, (pmx_material, count)) in pmx.materials.iter().zip(&counts).enumerate() {
        let material = builder.add_material(material_of(pmx_material, order as i32, &textures));
        submeshes.push(Submesh {
            name: pmx_material.name.clone(),
            first_index,
            index_count: *count,
            material: Some(material),
            ..Submesh::default()
        });
        first_index += count;
    }
    if first_index as usize != indices.len() {
        return Err(Error::Damaged(format!(
            "materials cover {first_index} of {} face indices",
            indices.len()
        )));
    }

    let morphs = targets.len();
    let mesh = builder.add_mesh(Mesh {
        vertices,
        indices,
        submeshes,
        morph_targets: targets,
        ..Mesh::new(pmx.name.clone())
    });
    let root = builder.add_node(
        Node::new(pmx.name.clone())
            .mesh(mesh)
            .morph_weights(vec![0.0; morphs]),
    );

    let nodes = attach_bones(&mut builder, &mut pmx, root);

    let skin = builder.add_skin(Skin {
        joints: nodes.clone(),
        inverse_bind: pmx
            .bones
            .iter()
            .map(|bone| rikka_model_core::glam::Mat4::from_translation(-bone.position))
            .collect(),
        skeleton: Some(root),
        ..Skin::new(pmx.name.clone())
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);

    pmx.mesh = mesh;
    pmx.root = root;
    if pmx.bones.is_empty() {
        builder.warn(Warning::Repaired("PMX has no bones".to_owned()));
    }
    builder.extend_with(pmx);
    builder.finish()
}

pub(in crate::pmx) fn attach_bones(
    builder: &mut ModelBuilder,
    pmx: &mut Pmx,
    root: NodeId,
) -> Vec<NodeId> {
    let nodes: Vec<NodeId> = pmx
        .bones
        .iter()
        .map(|bone| builder.add_node(Node::new(bone.name.clone())))
        .collect();
    let positions: Vec<Vec3> = pmx.bones.iter().map(|bone| bone.position).collect();
    for (index, bone) in pmx.bones.iter_mut().enumerate() {
        bone.node = nodes[index];
        let (parent, above) = match bone.parent {
            Some(parent) => (nodes[parent], positions[parent]),
            None => (root, Vec3::ZERO),
        };
        builder.attach(parent, nodes[index]);
        builder.model_mut().nodes[nodes[index].index()].transform =
            Transform::from_translation(bone.position - above);
    }
    nodes
}

fn bind_to_bones(vertices: &mut Vertices, bones: usize) -> bool {
    let mut repaired = false;
    for joints in &mut vertices.joints {
        for joint in joints {
            if usize::from(*joint) >= bones {
                *joint = 0;
                repaired = true;
            }
        }
    }
    repaired
}

fn load_pictures(
    pmx: &Pmx,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<Option<TextureId>>> {
    let paths: Vec<&str> = pmx.textures.iter().map(String::as_str).collect();
    ctx.prefetch_resources(&paths)?;
    let mut pictures = Vec::with_capacity(pmx.textures.len());
    for path in &pmx.textures {
        pictures.push(
            ctx.import_image(builder, &[path])?
                .map(|image| builder.add_texture(Texture::new(image).named(path.clone()))),
        );
    }
    Ok(pictures)
}

fn ramps_of(
    pmx: &Pmx,
    pictures: &[Option<TextureId>],
    builder: &mut ModelBuilder,
) -> Vec<Option<TextureId>> {
    let mut ramps = vec![None; pictures.len()];
    for pmx_material in &pmx.materials {
        let PmxToon::Texture(Some(index)) = pmx_material.toon else {
            continue;
        };
        let (Some(slot), Some(Some(picture))) = (ramps.get(index), pictures.get(index)) else {
            continue;
        };
        if slot.is_some() {
            continue;
        }
        let picture = builder.model_mut().textures[picture.index()].clone();
        ramps[index] = Some(builder.add_texture(clamped(picture)));
    }
    ramps
}

fn load_shared_ramps(
    pmx: &Pmx,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<[Option<TextureId>; SHARED_TOONS]> {
    let mut used = [false; SHARED_TOONS];
    for pmx_material in &pmx.materials {
        if let PmxToon::Shared(index) = pmx_material.toon
            && let Some(slot) = used.get_mut(usize::from(index))
        {
            *slot = true;
        }
    }
    let mut ramps = [None; SHARED_TOONS];
    for (index, ramp) in ramps.iter_mut().enumerate() {
        if !used[index] {
            continue;
        }
        let path = shared_toon_path(index);
        let bytes = match ctx.read_resource(&path) {
            Ok(bytes) => bytes,
            Err(Error::Read { .. }) => continue,
            Err(error) => return Err(error),
        };
        let image = builder.add_image(Image::new(path.clone(), bytes));
        *ramp = Some(builder.add_texture(clamped(Texture::new(image).named(path))));
    }
    Ok(ramps)
}

fn clamped(texture: Texture) -> Texture {
    texture.sampler(Sampler::new(Filter::Linear, Wrap::Clamp))
}
