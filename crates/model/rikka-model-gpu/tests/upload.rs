use std::sync::Arc;

use rikka_model_core::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_model_core::{
    Image, Instance, Material, MaterialTextures, Mesh, Model, ModelBuilder, Node, Sampler, Texture,
    Transform, Vertices,
};
use rikka_model_gpu::GpuModel;
use rikka_renderer::{Node as SceneNode, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{Background, Camera, Frame, Outline, Viewport, ViewportConfig};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 128;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-model-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn png(red: u8, green: u8, blue: u8) -> Vec<u8> {
    rikka_image::Image::from_rgba8(2, 2, [red, green, blue, 255].repeat(4))
        .expect("a 2x2 picture")
        .encode(rikka_image::Png)
        .expect("a png")
}

fn quad_model() -> Arc<Model> {
    let mut builder = ModelBuilder::new().importer("test");
    let image = builder.add_image(Image::new("red", png(255, 0, 0)));
    let texture = builder.add_texture(Texture::new(image).sampler(Sampler::default()));
    let material = builder.add_material(Material::new("red").textures(MaterialTextures {
        base_color: Some(texture),
        ..MaterialTextures::default()
    }));

    let half = 0.5;
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![
                Vec3::new(-half, -half, 0.0),
                Vec3::new(half, -half, 0.0),
                Vec3::new(half, half, 0.0),
                Vec3::new(-half, half, 0.0),
            ],
            normals: vec![Vec3::Z; 4],
            tangents: vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2, 0, 2, 3],
        submeshes: vec![rikka_model_core::Submesh {
            index_count: 6,
            material: Some(material),
            ..rikka_model_core::Submesh::default()
        }],
        ..Mesh::new("quad")
    });
    builder.add_node(Node::new("quad").mesh(mesh));

    Arc::new(builder.finish().expect("a quad is a model"))
}

fn frame(gpu: &GpuModel, instance: &Instance) -> Frame {
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ONE)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    gpu.draw(&mut frame, instance);
    frame
}

fn read(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
    renderer
        .three()
        .render(viewport, frame)
        .expect("a 3D frame draws");

    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(SceneNode::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    renderer.read_pixels(&mut target).expect("pixels come back")
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
    let i = (y * SIZE as usize + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

fn viewport(renderer: &mut Renderer) -> Viewport {
    renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1).identity(true))
        .expect("a viewport")
}

#[test]
fn uploaded_model_draws_texture_colour() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let model = quad_model();
    let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let instance = gpu.instance();

    let pixels = read(&mut renderer, &mut view, &frame(&gpu, &instance));
    let (r, g, b, a) = pixel(&pixels, 64, 64);
    assert!(
        r > 180 && g < 70 && b < 70 && a > 200,
        "middle: {r},{g},{b}"
    );
    assert_eq!(pixel(&pixels, 4, 4), (0, 0, 0, 255), "corner");

    gpu.destroy(&mut renderer);
}

#[test]
fn pick_returns_drawn_node() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let model = quad_model();
    let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let instance = gpu.instance();

    read(&mut renderer, &mut view, &frame(&gpu, &instance));

    let pick = renderer
        .three()
        .pick(&view, 64, 64, 1, 1)
        .expect("a picked pixel");
    let node = pick
        .at(64, 64)
        .and_then(|id| gpu.node_at(id))
        .expect("a node");
    assert_eq!(model.node(node).name, "quad");

    let empty = renderer
        .three()
        .pick(&view, 4, 4, 1, 1)
        .expect("a picked pixel");
    assert!(empty.at(4, 4).is_none());

    gpu.destroy(&mut renderer);
}

#[test]
fn skinned_mesh_follows_animated_bone() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);

    let model = Arc::new(skinned_model());
    let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let mut instance = gpu.instance();
    let animation = model.find_animation("aside").expect("the animation");
    instance
        .player_mut()
        .set_repeat(rikka_model_core::Repeat::Once);
    instance.play(animation);
    instance.advance(1.0);

    let pixels = read(&mut renderer, &mut view, &frame(&gpu, &instance));
    let (r, _, _, _) = pixel(&pixels, 64, 64);
    assert!(r < 70, "the bone carried the mesh out of the middle: {r}");

    gpu.destroy(&mut renderer);
}

fn skinned_model() -> Model {
    let mut builder = ModelBuilder::new().importer("test");
    let material = builder.add_material(Material::new("red"));
    let mesh = builder.add_mesh(Mesh {
        vertices: Vertices {
            positions: vec![
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.5, 0.5, 0.0),
                Vec3::new(-0.5, 0.5, 0.0),
            ],
            normals: vec![Vec3::Z; 4],
            joints: vec![[0, 0, 0, 0]; 4],
            weights: vec![Vec4::X; 4],
            ..Vertices::default()
        },
        indices: vec![0, 1, 2, 0, 2, 3],
        submeshes: vec![rikka_model_core::Submesh {
            index_count: 6,
            material: Some(material),
            ..rikka_model_core::Submesh::default()
        }],
        ..Mesh::new("quad")
    });

    let root = builder.add_node(Node::new("root").mesh(mesh));
    let bone = builder.add_node(Node::new("bone").transform(Transform::IDENTITY));
    builder.attach(root, bone);
    let skin = builder.add_skin(rikka_model_core::Skin {
        joints: vec![bone],
        inverse_bind: vec![Mat4::IDENTITY],
        ..rikka_model_core::Skin::new("skin")
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);
    builder.add_animation(rikka_model_core::Animation {
        name: "aside".into(),
        channels: vec![rikka_model_core::Channel {
            target: bone,
            track: rikka_model_core::Track::Translation(rikka_model_core::Curve::linear(
                vec![0.0, 1.0],
                vec![Vec3::ZERO, Vec3::new(-2.0, 0.0, 0.0)],
            )),
        }],
    });
    builder.finish().expect("a skinned quad is a model")
}

#[test]
fn dropped_model_is_freed_next_frame() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let model = quad_model();
    let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let named = model.nodes[0].mesh.expect("the quad's node names a mesh");
    let mesh = gpu.mesh(named).expect("the quad is uploaded");
    let instance = gpu.instance();
    let _ = read(&mut renderer, &mut view, &frame(&gpu, &instance));

    drop(instance);
    drop(gpu);
    assert!(
        !renderer.reclaimer().is_empty(),
        "what was dropped away from the renderer is left with it"
    );

    renderer.reclaim();

    assert!(renderer.reclaimer().is_empty());
    assert!(
        !renderer
            .three()
            .destroy_mesh(mesh)
            .expect("a renderer with mesh support"),
        "the model's mesh was let go of without another one being drawn first"
    );
}

const SWIZZLED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaNoData> {
    [Texture("colour")] [Role("albedo_texture")] RikkaTexture picture;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        RikkaSurfaceData<RikkaNoData> s = rikka_surface_data(pixel, data);
        s.albedo = picture.sampled(pixel.uv).gbr;
        return s;
    }
};

typealias Model = RikkaUnlitModel;
"#;

#[test]
fn restyled_models_draw_through_the_template_with_their_textures() {
    let mut renderer = renderer();
    let mut view = viewport(&mut renderer);
    let model = quad_model();
    let gpu = GpuModel::upload(&mut renderer, Arc::clone(&model)).expect("an uploaded model");
    let instance = gpu.instance();
    let compiler = rikka_renderer_3d::ShaderCompiler::new().expect("libslang loads");
    let template = compiler
        .template("swizzled", SWIZZLED)
        .expect("the template compiles");
    let restyle = gpu
        .restyle(&mut renderer, &template)
        .expect("the model restyles");
    assert_eq!(restyle.template(), &template);

    let mut frame = frame_without_model();
    gpu.drawing(&instance)
        .restyled(&restyle)
        .draw_into(&mut frame);
    let pixels = read(&mut renderer, &mut view, &frame);
    let (r, g, b, _) = pixel(&pixels, 64, 64);
    assert!(
        b > 180 && r < 70 && g < 70,
        "the red texture comes out blue: {r},{g},{b}"
    );
    drop(restyle);
    gpu.destroy(&mut renderer);
}

fn frame_without_model() -> Frame {
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    );
    let mut frame = Frame::new(camera);
    frame
        .background(Background::Transparent)
        .ambient(Vec3::ONE)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}
