use glam::{Vec3, Vec4};
use rikka_mesh_modifiers::{
    Displace, DisplaceDirection, DisplaceSpace, Texture, TextureCoordinates, TextureRef,
    TextureSample,
};
use rikka_polymesh::Polymesh;

use crate::oracle::scene::{
    DISPLACE_NORMAL, DISPLACE_TEXTURE_GLOBAL_GROUP, DISPLACE_TEXTURE_LOCAL, DISPLACE_TEXTURE_UV,
    DISPLACE_X_GLOBAL,
};
use crate::support::{GRID, Probe, Scene, assert_close, fixture, placement, run_in, with_uvs};

struct LinearBlend;

impl Texture for LinearBlend {
    fn sample(&self, coordinate: Vec3) -> TextureSample {
        let intensity = (1.0 + coordinate.x) / 2.0;
        TextureSample {
            intensity,
            color: Vec4::new(intensity, intensity, intensity, 1.0),
        }
    }
}

fn grid() -> Polymesh {
    let mut mesh = with_uvs(fixture(&GRID));
    let mask = mesh.vertex_groups.add("Mask");
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for point in 0..mesh.point_count() {
        mesh.vertex_groups
            .set_weight(point, mask, point as f32 / 8.0);
    }
    mesh
}

fn scene() -> Scene {
    Scene {
        matrix: placement([0.3, -0.2, 0.1], [0.4, 0.0, 0.7], [1.0, 1.5, 1.0]),
        textures: vec![Box::new(LinearBlend)],
        ..Scene::default()
    }
}

#[test]
fn displace_matches_blender() {
    let textured = Some(TextureRef::new(0));
    let cases: [(Displace, &Probe); 5] = [
        (
            Displace {
                strength: 0.5,
                mid_level: 0.25,
                ..Displace::default()
            },
            &DISPLACE_NORMAL,
        ),
        (
            Displace {
                direction: DisplaceDirection::X,
                space: DisplaceSpace::Global,
                strength: 0.3,
                ..Displace::default()
            },
            &DISPLACE_X_GLOBAL,
        ),
        (
            Displace {
                texture: textured,
                direction: DisplaceDirection::Z,
                strength: 0.7,
                mid_level: 0.4,
                ..Displace::default()
            },
            &DISPLACE_TEXTURE_LOCAL,
        ),
        (
            Displace {
                texture: textured,
                texture_coords: TextureCoordinates::Global,
                strength: 1.5,
                vertex_group: String::from("Mask"),
                ..Displace::default()
            },
            &DISPLACE_TEXTURE_GLOBAL_GROUP,
        ),
        (
            Displace {
                texture: textured,
                texture_coords: TextureCoordinates::Uv,
                direction: DisplaceDirection::Y,
                strength: 0.4,
                ..Displace::default()
            },
            &DISPLACE_TEXTURE_UV,
        ),
    ];
    let scene = scene();
    for (displace, probe) in cases {
        let evaluated = run_in(&grid(), displace, &scene).mesh;
        assert_close(&evaluated.positions, probe.positions);
    }
}

#[test]
fn missing_texture_skips_modifier() {
    let displace = Displace {
        texture: Some(TextureRef::new(5)),
        ..Displace::default()
    };
    let stack = [rikka_mesh_modifiers::Modifier::new("Displace", displace)];
    let mesh = grid();
    let evaluated = rikka_mesh_modifiers::evaluate(
        &mesh,
        &stack,
        &scene(),
        rikka_mesh_modifiers::Purpose::Viewport,
    );
    assert_eq!(evaluated.mesh.positions, mesh.positions);
    assert_eq!(
        evaluated.notes[0].kind,
        rikka_mesh_modifiers::NoteKind::Skipped(rikka_mesh_modifiers::SkipReason::MissingTexture)
    );
}
