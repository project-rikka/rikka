use crate::support::{SIZE, ambient_frame, flat, quad, read, renderer, viewport};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Draw;
use rikka_renderer_3d::glam::Mat4;

#[test]
fn msaa_viewport_antialiases_edges() {
    let mesh_and_frame = |renderer: &mut Renderer| {
        let mesh = quad(renderer, 0.0, 0.5);
        let material = flat(renderer, [1.0, 1.0, 1.0, 1.0]);
        let mut frame = ambient_frame(1.0);
        frame.draw(Draw::new(mesh, material).transform(Mat4::from_rotation_z(0.3)));
        frame
    };

    let mut renderer = renderer();

    let mut single = viewport(&mut renderer, 1);
    let frame = mesh_and_frame(&mut renderer);
    let flat_pixels = read(&mut renderer, &mut single, &frame);

    let mut multi = viewport(&mut renderer, 4);
    let multi_pixels = read(&mut renderer, &mut multi, &frame);

    let partial = |pixels: &[u8]| {
        (0..SIZE as usize * SIZE as usize)
            .filter(|i| {
                let value = pixels[i * 4];
                (16..240).contains(&value)
            })
            .count()
    };
    let hard = partial(&flat_pixels);
    let soft = partial(&multi_pixels);
    assert!(
        soft > hard,
        "four samples soften more edge pixels than one: {soft} vs {hard}"
    );
}
