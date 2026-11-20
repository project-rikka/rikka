use std::sync::Arc;

use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{
    Confidence, CoordinateSystem, ExportCtx, ExportOptions, Exporter, ExporterInfo, Image,
    ImportCtx, Importer, ImporterInfo, Input, Mesh, Model, ModelBuilder, Node, Registry, Result,
    Skin, Supports, Transform, Vertices,
};

const HEADER: &str = "TOY";

pub(crate) const TEXTURE: &str = "Textures\\Skin.png";

pub(crate) struct Toy {
    pub(crate) supports: Supports,
}

impl Exporter for Toy {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: "toy",
            extensions: &["toy"],
            supports: self.supports,
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        ctx.checkpoint()?;
        let mut lines = vec![format!("{HEADER} {}", model.metadata.unit_scale)];
        for node in &model.nodes {
            let place = node.transform.translation;
            lines.push(format!(
                "n {} {} {} {}",
                node.name, place.x, place.y, place.z
            ));
        }
        for mesh in &model.meshes {
            for vertex in &mesh.vertices.positions {
                lines.push(format!("v {} {} {}", vertex.x, vertex.y, vertex.z));
            }
        }
        for image in &model.images {
            match ctx.embeds() {
                true => lines.push(format!("i {}", image.name)),
                false => {
                    ctx.write_resource(&image.name, &image.bytes)?;
                    lines.push(format!("r {}", image.name));
                }
            }
        }
        Ok(lines.join("\n").into_bytes())
    }
}

impl Importer for Toy {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: "toy",
            extensions: &["toy"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        match head.starts_with(HEADER.as_bytes()) {
            true => Confidence::Yes,
            false => Confidence::No,
        }
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        ctx.checkpoint()?;
        let text = String::from_utf8_lossy(input.bytes()).into_owned();
        let mut lines = text.lines();
        let unit_scale = lines
            .next()
            .and_then(|header| header.split_whitespace().nth(1))
            .and_then(|scale| scale.parse().ok())
            .unwrap_or(1.0);

        let mut builder = ModelBuilder::new()
            .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
            .unit_scale(unit_scale);
        let mut positions = Vec::new();
        let mut place = Vec3::ZERO;
        let mut name = String::from("root");

        for line in lines {
            let mut parts = line.split_whitespace();
            let kind = parts.next().unwrap_or_default();
            if kind == "n" {
                name = parts.next().unwrap_or_default().to_owned();
            }
            if kind == "r" {
                let uri = parts.next().unwrap_or_default();
                if let Some(bytes) = ctx.read_optional_resource(uri)? {
                    builder.add_image(Image::new(uri, bytes.to_vec()));
                }
                continue;
            }
            let numbers: Vec<f32> = parts.filter_map(|value| value.parse().ok()).collect();
            let Ok([x, y, z]) = <[f32; 3]>::try_from(numbers) else {
                continue;
            };
            match kind {
                "n" => place = Vec3::new(x, y, z),
                "v" => positions.push(Vec3::new(x, y, z)),
                _ => {}
            }
        }

        let mesh = builder.add_mesh(Mesh {
            vertices: Vertices {
                positions,
                ..Vertices::default()
            },
            indices: vec![0, 1, 2],
            ..Mesh::new("triangle")
        });
        builder.add_node(
            Node::new(name)
                .transform(Transform::from_translation(place))
                .mesh(mesh),
        );
        builder.finish()
    }
}

pub(crate) fn registry() -> Arc<Registry> {
    keeping(Supports::all())
}

pub(crate) fn keeping(supports: Supports) -> Arc<Registry> {
    Arc::new(
        Registry::new()
            .with_importer(Toy { supports })
            .with_exporter(Toy { supports }),
    )
}

pub(crate) fn toy_options() -> ExportOptions {
    ExportOptions::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(0.1)
}

pub(crate) fn model() -> Model {
    let mut builder = ModelBuilder::new().importer("test");
    builder.add_image(Image::new(TEXTURE, b"pixels".to_vec()));
    let mesh = builder.add_mesh(triangle());
    let node = builder.add_node(
        Node::new("root")
            .transform(Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)))
            .mesh(mesh),
    );
    builder.add_skin(Skin {
        joints: vec![node],
        inverse_bind: vec![Mat4::IDENTITY],
        ..Skin::new("skin")
    });
    builder.finish().expect("a model")
}

pub(crate) fn triangle() -> Mesh {
    Mesh {
        vertices: Vertices {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::new(0.0, 2.0, 3.0)],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2],
        ..Mesh::new("triangle")
    }
}
