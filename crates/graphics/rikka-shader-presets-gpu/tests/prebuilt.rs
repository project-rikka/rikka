use std::collections::BTreeSet;

use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    Background, Draw, Frame, Light, MaterialDesc, MaterialState, MaterialTemplate, ShaderCompiler,
    ShadowQuality, TemplateReadiness,
};
use rikka_shader_graph::Library;

use crate::support::{Shot, create, frame, preset, renderer, texture};

const DRAFT_TOLERANCE: i16 = 2;

#[test]
fn every_preset_is_prebuilt() {
    let prebuilt: BTreeSet<&str> = rikka_shader_presets_gpu::keys().collect();
    let listed: BTreeSet<&str> = rikka_shader_presets::presets()
        .iter()
        .map(|preset| preset.key)
        .collect();
    assert_eq!(prebuilt, listed);
}

fn scene(renderer: &mut Renderer, template: &MaterialTemplate) -> Frame {
    let ramp = texture(renderer, 1, 2, |_, y| match y {
        0 => [255, 255, 255, 255],
        _ => [180, 120, 200, 255],
    });
    let sphere = texture(renderer, 2, 2, |x, y| [40 * x as u8, 60 * y as u8, 90, 255]);
    let schema = template.schema();
    let mut desc = MaterialDesc::new(template).state(MaterialState::opaque());
    let values = [
        ("base_color", Vec4::new(0.7, 0.5, 0.4, 1.0).into()),
        ("shade_color", Vec3::new(0.3, 0.2, 0.3).into()),
        ("specular_color", Vec3::splat(0.5).into()),
        ("rim_color", Vec3::new(0.2, 0.3, 0.6).into()),
        ("toon_texture", ramp),
        ("sphere_texture", sphere.clone()),
        ("matcap_texture", sphere),
    ];
    for (name, value) in values {
        if schema.parameter(name).is_some() {
            desc.set(name, value).expect("a declared parameter");
        }
    }
    let material = create(renderer, desc);
    let ball = rikka_test_renderer_3d::sphere(renderer, 1.0, 32).expect("a sphere");
    let mut frame = frame(None);
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(0.2))
        .light(
            Light::directional(Vec3::new(-0.6, -0.4, -0.7))
                .intensity(1.2)
                .shadow(ShadowQuality::None),
        )
        .draw(Draw::new(ball, material));
    frame
}

fn close(a: &Shot, b: &Shot) -> bool {
    a.bytes()
        .iter()
        .zip(b.bytes())
        .all(|(a, b)| (i16::from(*a) - i16::from(*b)).abs() <= DRAFT_TOLERANCE)
}

#[test]
fn runtime_compiled_presets_draw_like_prebuilt_ones() {
    let mut renderer = renderer();
    let library = Library::standard();
    let compiler = ShaderCompiler::new().expect("libslang loads");
    for key in ["mmd", "mtoon"] {
        let source = rikka_shader_presets::preset(key)
            .expect("a listed preset")
            .graph(&library)
            .and_then(|graph| graph.material(&library, key))
            .expect("the preset generates a material");
        let compiled = compiler
            .template(key, &source.source)
            .expect("the preset compiles at run time");
        let template = preset(key);
        let prebuilt = scene(&mut renderer, &template);
        let prebuilt = Shot::take(&mut renderer, &prebuilt);
        assert!(
            matches!(template.readiness(), TemplateReadiness::Built),
            "{key}: a prebuilt template is built once it draws"
        );
        let runtime = scene(&mut renderer, &compiled);
        let drafted = Shot::take(&mut renderer, &runtime);
        let optimized =
            rikka_test_renderer_3d::built(&compiled, || Shot::take(&mut renderer, &runtime));
        assert!(
            prebuilt.bytes().iter().any(|channel| *channel > 0),
            "{key} draws something"
        );
        assert!(
            close(&prebuilt, &drafted),
            "{key}: the run-time draft draws like the prebuilt template"
        );
        assert_eq!(
            prebuilt.bytes(),
            optimized.bytes(),
            "{key}: the prebuilt and the optimized run-time template draw alike"
        );
    }
}
