use std::path::{Path, PathBuf};

use rikka_renderer::{Node, Scene};
use rikka_types::{Color, Rect};

const HEADER_SIZE: usize = 32;

fn draw_and_save() {
    let mut renderer = rikka_test_renderer::renderer("rikka-pipeline-cache-test")
        .expect("a Vulkan 1.3 device is required for these tests");
    let mut target = renderer.create_target(64, 64).expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.2, 0.4, 0.6));
    scene.insert(Node::rect(Rect::new(8.0, 8.0, 32.0, 32.0)).fill(Color::rgb(1.0, 0.5, 0.0)));
    let out = renderer
        .draw(&mut scene, &mut target)
        .expect("the scene draws");
    assert!(out.rendered());
    renderer
        .gpu()
        .save_pipeline_cache()
        .expect("the pipeline cache saves");
}

fn saved_caches(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("pipelines-") && name.ends_with(".bin"))
        })
        .collect()
}

#[test]
fn pipeline_cache_persists_across_renderers() {
    rikka_test_renderer::init_tracing();
    let home = std::env::temp_dir().join(format!("rikka-pipeline-cache-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("a scratch cache home");
    unsafe { std::env::set_var("XDG_CACHE_HOME", &home) };
    let directory = home.join("rikka").join("renderer");

    draw_and_save();
    let [cache] = saved_caches(&directory)
        .try_into()
        .unwrap_or_else(|found: Vec<PathBuf>| {
            panic!(
                "expected one pipeline cache under {}, found {found:?}",
                directory.display()
            )
        });
    let saved = std::fs::read(&cache).expect("the saved cache reads");
    assert!(
        saved.len() > HEADER_SIZE,
        "the saved cache holds pipelines, not just a header"
    );
    let header_size = u32::from_le_bytes(saved[0..4].try_into().expect("four bytes")) as usize;
    let version = u32::from_le_bytes(saved[4..8].try_into().expect("four bytes"));
    assert!((HEADER_SIZE..=saved.len()).contains(&header_size));
    assert_eq!(version, 1, "the cache carries a version-one header");

    draw_and_save();
    assert_eq!(
        saved_caches(&directory),
        vec![cache.clone()],
        "a second renderer reuses the same cache file"
    );
    let reread = std::fs::read(&cache).expect("the cache still reads");
    assert!(
        reread.len() >= saved.len(),
        "restoring the cache keeps what was saved"
    );

    std::fs::remove_dir_all(&home).expect("the scratch cache home is removed");
}
