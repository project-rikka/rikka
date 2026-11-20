use rikka_renderer::Renderer;
use rikka_renderer_3d::Result;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    BuiltinTemplate, Composite, Environment, Exposure, Grade, Indices, Look, MaterialDesc,
    MaterialState, MaterialTemplate, MeshData, MeshId, Operation, Sky, Tonemap,
};

pub fn sphere(renderer: &mut Renderer, radius: f32, rings: u32) -> Result<MeshId> {
    let segments = rings * 2;
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for ring in 0..=rings {
        let polar = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            normals.push(Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.cos(),
                polar.sin() * azimuth.sin(),
            ));
            uvs.push(Vec2::new(
                segment as f32 / segments as f32,
                ring as f32 / rings as f32,
            ));
        }
    }
    let positions: Vec<Vec3> = normals.iter().map(|normal| *normal * radius).collect();
    let tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); positions.len()];
    let mut indices = Vec::new();
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let b = a + row;
            indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    renderer.three().create_mesh(&MeshData {
        positions: &positions,
        normals: &normals,
        tangents: &tangents,
        uvs: &uvs,
        indices: Some(Indices::U32(&indices)),
        ..MeshData::default()
    })
}

pub fn sky_of(look: &mut Look) -> &mut Sky {
    if look.environment.sky().is_none() {
        look.environment = Environment::Sky(Sky::default());
    }
    match &mut look.environment {
        Environment::Sky(sky) => sky,
        Environment::None | Environment::Image(_) => unreachable!("the look was given a sky"),
    }
}

pub fn minimal_look() -> Look {
    let mut look = Look::default();
    sky_of(&mut look).clouds = 0.0;
    sky_of(&mut look).stars = 0.0;
    look.fog.density = 0.0;
    look.fog.shafts = 0.0;
    look.occlusion.strength = 0.0;
    look.reflections.strength = 0.0;
    look.illumination.strength = 0.0;
    look.lines.strength = 0.0;
    look.particles.amount = 0.0;
    look.composite = minimal_composite();
    look
}

pub fn minimal_composite() -> Composite {
    let mut composite = Composite::new();
    composite.push(Exposure::default());
    composite.push(Grade::default());
    composite.push(Tonemap::default());
    composite
}

pub fn exposure_mut(look: &mut Look) -> &mut Exposure {
    look.composite
        .nodes
        .iter_mut()
        .find_map(|node| match &mut node.operation {
            Operation::Exposure(exposure) => Some(exposure),
            _ => None,
        })
        .expect("the composite exposes the image")
}

pub fn standard(base_color: Vec4, state: MaterialState) -> MaterialDesc {
    MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Standard))
        .state(state)
        .with("base_color", base_color.truncate())
        .and_then(|desc| desc.with("geometry_opacity", base_color.w))
        .and_then(|desc| desc.with("specular_roughness", 1.0))
        .expect("the standard template has a base colour, an opacity and a roughness")
}
