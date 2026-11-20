use std::path::PathBuf;
use std::sync::Arc;

use rikka_blend::{BlendImporter, BlendScene, read_scene};
use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{
    DirectoryResolver, Import, ImportOptions, Input, Model, NodeId, Pose, Registry, Result,
};
use rikka_types::CancelToken;

const ASSETS: &str = "crates/model/rikka-blend/assets";

pub(crate) fn asset(name: &str) -> PathBuf {
    PathBuf::from(ASSETS).join(name)
}

pub(crate) fn registry() -> Arc<Registry> {
    Arc::new(Registry::new().with_importer(BlendImporter))
}

pub(crate) fn scene_from(name: &str, bytes: &[u8]) -> Result<BlendScene> {
    let path = asset(name);
    read_scene(
        &Input::new(bytes, Some(&path)),
        &DirectoryResolver::new(ASSETS),
        &ImportOptions::default(),
        &CancelToken::new(),
    )
}

pub(crate) fn scene(name: &str) -> BlendScene {
    let bytes = std::fs::read(asset(name)).expect("the asset exists");
    scene_from(name, &bytes).expect("the Blender file is read as a scene")
}

pub(crate) fn keep_output(name: &str, bytes: &[u8]) {
    if let Some(folder) = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR") {
        std::fs::write(PathBuf::from(folder).join(name), bytes).expect("the output is written");
    }
}

pub(crate) fn read(name: &str) -> Model {
    Import::open(asset(name), registry())
        .run()
        .expect("the Blender file is read")
}

#[derive(Debug)]
pub(crate) struct Shape {
    pub(crate) triangles: usize,
    pub(crate) area: f32,
    pub(crate) low: Vec3,
    pub(crate) high: Vec3,
}

pub(crate) fn shapes(model: &Model, time: Option<f32>) -> Vec<(String, Shape)> {
    let mut pose = Pose::rest(model);
    if let (Some(time), Some(animation)) = (time, model.animations.first()) {
        animation.apply(time, &mut pose);
    }
    let mut worlds: Vec<Mat4> = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        let local = pose.local(NodeId(index as u32)).to_matrix();
        worlds.push(match node.parent {
            Some(parent) => worlds[parent.index()] * local,
            None => local,
        });
    }
    let blender = |point: Vec3| Vec3::new(point.x, -point.z, point.y);
    let mut shapes = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh else {
            continue;
        };
        let mesh = model.mesh(mesh);
        let mut positions = mesh.vertices.positions.clone();
        for (target, weight) in mesh
            .morph_targets
            .iter()
            .zip(pose.weights(NodeId(index as u32)))
        {
            for (&vertex, offset) in target.vertices.iter().zip(&target.positions) {
                positions[vertex as usize] += *offset * *weight;
            }
        }
        let skin = node.skin.map(|skin| &model.skins[skin.index()]);
        let world: Vec<Vec3> = positions
            .iter()
            .enumerate()
            .map(|(vertex, &point)| {
                let matrix = match skin {
                    Some(skin) => (0..4).fold(Mat4::ZERO, |sum, slot| {
                        let joint = mesh.vertices.joints[vertex][slot] as usize;
                        sum + worlds[skin.joints[joint].index()]
                            * skin.inverse_bind[joint]
                            * mesh.vertices.weights[vertex][slot]
                    }),
                    None => worlds[index],
                };
                blender(matrix.transform_point3(point))
            })
            .collect();
        let area = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle.map(|vertex| world[vertex as usize]);
                (b - a).cross(c - a).length() / 2.0
            })
            .sum();
        shapes.push((
            node.name.clone(),
            Shape {
                triangles: mesh.indices.len() / 3,
                area,
                low: world.iter().copied().fold(Vec3::MAX, Vec3::min),
                high: world.iter().copied().fold(Vec3::MIN, Vec3::max),
            },
        ));
    }
    shapes
}

pub(crate) type Expected<'a> = (&'a str, usize, f32, [f32; 3], [f32; 3]);

pub(crate) fn assert_shapes(model: &Model, time: Option<f32>, expected: &[Expected<'_>]) {
    let shapes = shapes(model, time);
    for &(name, triangles, area, low, high) in expected {
        let (_, shape) = shapes
            .iter()
            .find(|(entry_name, _)| entry_name == name)
            .unwrap_or_else(|| panic!("{name} is missing from {shapes:?}"));
        assert_eq!(shape.triangles, triangles, "{name} triangles");
        assert!(
            (shape.area / area - 1.0).abs() < 1.0e-4,
            "{name} area {} is not {area}",
            shape.area
        );
        assert!(
            shape.low.abs_diff_eq(Vec3::from_array(low), 1.0e-3)
                && shape.high.abs_diff_eq(Vec3::from_array(high), 1.0e-3),
            "{name} bounds {} to {} are not {low:?} to {high:?}",
            shape.low,
            shape.high
        );
    }
}
