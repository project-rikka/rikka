use glam::{Vec2, Vec4};
use rikka_file_format::{Codec, Reader, Writer};
use rikka_polymesh::{Attribute, Domain, Error, Polymesh, ShapeKey, Values};

use crate::support::cube;

fn rich() -> Polymesh {
    let mut mesh = cube();
    mesh.add_uv_map(
        "UVMap",
        (0..24).map(|corner| Vec2::splat(corner as f32)).collect(),
    );
    mesh.uv_pins_mut("UVMap")[3] = true;
    mesh.add_color_attribute("Col", Domain::Point, vec![Vec4::new(0.1, 0.2, 0.3, 1.0); 8]);
    mesh.material_indices_mut()[2] = 1;
    mesh.edge_creases_mut()[5] = 0.5;
    mesh.attributes.insert(Attribute::new(
        "id",
        Domain::Edge,
        Values::Int((0..12).collect()),
    ));
    let group = mesh.vertex_groups.add("Spine");
    mesh.vertex_groups.ensure_weights(8);
    mesh.vertex_groups.set_weight(4, group, 0.25);
    mesh.vertex_groups.groups[0].locked = true;
    mesh.shape_keys
        .add(ShapeKey::new("Basis", mesh.positions.clone()));
    let mut key = ShapeKey::new("Smile", mesh.positions.clone());
    key.vertex_group = Some("Spine".into());
    key.value = 0.75;
    key.muted = true;
    mesh.shape_keys.add(key);
    mesh
}

fn encode(mesh: &Polymesh) -> Vec<u8> {
    let mut bytes = Vec::new();
    mesh.encode(&mut Writer::new(&mut bytes));
    bytes
}

fn decode(bytes: &[u8]) -> Result<Polymesh, Error> {
    let mut reader = Reader::new(bytes);
    let mesh = Polymesh::decode(&mut reader)?;
    reader.finish()?;
    Ok(mesh)
}

#[test]
fn codec_round_trips_every_layer() {
    let mesh = rich();
    assert_eq!(decode(&encode(&mesh)).expect("encoded meshes decode"), mesh);
}

#[test]
fn codec_refuses_truncated_and_hostile_input() {
    let bytes = encode(&rich());
    for cut in [0, 7, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode(&bytes[..cut]).is_err(), "truncated at {cut}");
    }
    let mut hostile = Vec::new();
    {
        let mut writer = Writer::new(&mut hostile);
        writer.count64(usize::MAX / 2);
    }
    hostile.extend_from_slice(&[0; 64]);
    assert!(matches!(decode(&hostile), Err(Error::Decode(_))));
}

#[test]
fn codec_refuses_broken_structure() {
    let mut mesh = cube();
    mesh.corner_verts[0] = 40;
    assert!(matches!(decode(&encode(&mesh)), Err(Error::Invalid(_))));
}
