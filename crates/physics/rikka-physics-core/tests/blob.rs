use bytemuck::{Pod, Zeroable};
use rikka_physics_core::{Blob, BlobWriter, Fnv1a, Section};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Header {
    points: Section<[f32; 3]>,
    indices: Section<u32>,
}

#[test]
fn blob_round_trips_and_rejects_corruption() {
    let mut writer = BlobWriter::<Header>::new();
    let points = writer.push(&[[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]);
    let indices = writer.push(&[0u32, 1, 1]);
    let blob = writer.finish(Header { points, indices });

    let copy = Blob::<Header>::from_bytes(blob.bytes()).unwrap();
    assert_eq!(copy, blob);
    assert_eq!(copy.section(copy.header().points)[1], [3.0, 4.0, 5.0]);
    assert_eq!(copy.section(copy.header().indices), &[0, 1, 1]);

    let mut header = *blob.header();
    header.indices.len = 1000;
    assert!(blob.try_section(header.indices).is_err());
    assert!(Blob::<Header>::from_bytes(&[0; 4]).is_err());
}

#[test]
fn hash_matches_fnv1a_over_le_words() {
    let mut hash = Fnv1a::new();
    hash.u32(0x0403_0201);
    let expected = (0xcbf2_9ce4_8422_2325_u64 ^ 0x0403_0201).wrapping_mul(0x0000_0100_0000_01b3);
    assert_eq!(hash.finish(), expected);

    let mut split = Fnv1a::new();
    split.u64(0x0000_0002_0000_0001);
    let mut words = Fnv1a::new();
    words.u32(1);
    words.u32(2);
    assert_eq!(split.finish(), words.finish());
}
