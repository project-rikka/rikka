use std::sync::Arc;

use rikka_model::{Import, MaterialId, Model, Shading, registry};
use rikka_model_gpu::{GpuModel, MaterialStyle, class_preset};

const MIKU: &[u8] = include_bytes!("../../rikka-mmd/assets/mmd/miku.pmx");
const AVATAR: &[u8] = include_bytes!("../../rikka-vrm/assets/avatar.vrm");

fn read(bytes: &[u8], name: &str) -> Arc<Model> {
    let model = Import::named_bytes(name, bytes.to_vec(), registry())
        .run()
        .unwrap_or_else(|error| panic!("{name} cannot be read: {error}"));
    Arc::new(model)
}

#[test]
fn toon_materials_shade_with_the_preset_of_their_format() {
    let mut renderer = rikka_test_renderer::renderer_3d("rikka-model-presets-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    for (bytes, name, key) in [(MIKU, "miku.pmx", "mmd"), (AVATAR, "avatar.vrm", "mtoon")] {
        let model = read(bytes, name);
        let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
        let mut toon = 0;
        for (index, material) in model.materials.iter().enumerate() {
            let id = MaterialId(index as u32);
            match material.shading {
                Shading::Toon(shading) => {
                    toon += 1;
                    assert_eq!(gpu.preset(id), Some(key), "{name}: {}", material.name);
                    assert_eq!(gpu.class(id), Some(shading.surface), "{name}");
                    assert_eq!(
                        gpu.is_outlined(id),
                        shading.outline.width.is_drawn(),
                        "{name}: {}",
                        material.name
                    );
                }
                Shading::Pbr(_) => {
                    assert_eq!(gpu.preset(id), None, "{name}: {}", material.name);
                    assert_eq!(gpu.class(id), None, "{name}");
                }
            }
        }
        assert!(toon > 0, "{name} has toon materials");
        gpu.destroy(&mut renderer);
    }
}

#[test]
fn restyled_materials_keep_their_class_and_outline() {
    let mut renderer = rikka_test_renderer::renderer_3d("rikka-model-presets-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    let model = read(MIKU, "miku.pmx");
    let mut gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let anime = rikka_shader_presets_gpu::template("anime").expect("a prebuilt anime preset");
    let outlined = (0..model.materials.len())
        .map(|index| MaterialId(index as u32))
        .find(|id| gpu.is_outlined(*id))
        .unwrap_or(MaterialId(0));
    let class = gpu.class(outlined).expect("a toon material");
    let was_outlined = gpu.is_outlined(outlined);
    let preset = class_preset(class).preset();
    gpu.apply_style(
        &mut renderer,
        outlined,
        MaterialStyle {
            template: Some(&anime),
            preset: Some(preset),
            values: &[],
            outline: None,
        },
    )
    .expect("the material restyles");
    assert_eq!(gpu.preset(outlined), Some("anime"));
    assert_eq!(gpu.class(outlined), Some(class));
    assert_eq!(gpu.is_outlined(outlined), was_outlined);
    gpu.destroy(&mut renderer);
}

#[test]
fn styled_outlines_follow_the_style_over_the_model_edge() {
    let mut renderer = rikka_test_renderer::renderer_3d("rikka-model-presets-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests");
    let model = read(MIKU, "miku.pmx");
    let mut gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let mtoon = rikka_shader_presets_gpu::template("mtoon").expect("a prebuilt mtoon preset");
    let ids: Vec<MaterialId> = (0..model.materials.len())
        .map(|index| MaterialId(index as u32))
        .collect();
    let bare = *ids
        .iter()
        .find(|id| !gpu.is_outlined(**id))
        .expect("a material without an edge");
    let edged = *ids
        .iter()
        .find(|id| gpu.is_outlined(**id))
        .expect("a material with an edge");
    for (material, outline, drawn) in [(bare, Some(true), true), (edged, Some(false), false)] {
        gpu.apply_style(
            &mut renderer,
            material,
            MaterialStyle {
                template: Some(&mtoon),
                outline,
                ..MaterialStyle::default()
            },
        )
        .expect("the material restyles");
        assert_eq!(gpu.is_outlined(material), drawn, "{outline:?}");
    }
    gpu.destroy(&mut renderer);
}
