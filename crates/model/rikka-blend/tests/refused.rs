use std::sync::Arc;

use rikka_blend::{BlendImporter, BlendOptions, write_scene};
use rikka_model_core::{Confidence, Import, Importer, Warning};
use rikka_model_scene::{Empty, Object, ObjectData, Parent, Scene};

use crate::support::{asset, registry, scene_from};

#[test]
fn only_blender_files_are_claimed() {
    let scene = std::fs::read(asset("scene.blend")).expect("the asset exists");
    let old =
        std::fs::read(asset("TorusLightsCams_250_compressed.blend")).expect("the asset exists");
    let mut packed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut packed, b"solid not a blend file").expect("gzip");
    let packed = packed.finish().expect("gzip");
    let cases: [(&[u8], Confidence); 4] = [
        (b"BLENDER-v280RENDH", Confidence::Yes),
        (&scene, Confidence::Yes),
        (&old, Confidence::Yes),
        (&packed, Confidence::No),
    ];
    for (head, expected) in cases {
        assert_eq!(BlendImporter.sniff(&head[..head.len().min(4096)]), expected);
    }
}

fn unpacked_torus() -> Vec<u8> {
    let scene =
        std::fs::read(asset("TorusLightsCams_250_compressed.blend")).expect("the asset exists");
    let mut unpacked = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&scene[..]), &mut unpacked)
        .expect("gzip");
    unpacked
}

#[test]
fn corrupt_files_fail() {
    let unpacked = unpacked_torus();
    let cases: [&[u8]; 3] = [
        b"BLENDER",
        b"BLENDER-v250REND",
        &unpacked[..unpacked.len() / 2],
    ];
    for bytes in cases {
        let result =
            Import::named_bytes("damaged.blend", Arc::<[u8]>::from(bytes), registry()).run();
        assert!(result.is_err(), "{} bytes were accepted", bytes.len());
    }
}

const FILE_HEADER: usize = 12;
const BLOCK_HEADER: usize = 24;
const FACE: usize = 20;
const TORUS_FACES: usize = 2304;

fn faces(bytes: &[u8]) -> std::ops::Range<usize> {
    let word =
        |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes")) as usize;
    let mut at = FILE_HEADER;
    loop {
        let (length, count) = (word(at + 4), word(at + 20));
        let start = at + BLOCK_HEADER;
        if &bytes[at..at + 4] == b"DATA" && count == TORUS_FACES && length == count * FACE {
            return start..start + length;
        }
        at = start + length;
    }
}

#[test]
fn faces_naming_missing_vertices_are_dropped() {
    let read = |bytes: Vec<u8>| {
        Import::named_bytes("torus.blend", bytes, registry())
            .run()
            .expect("the torus is read")
    };
    let intact = unpacked_torus();
    let mut damaged = intact.clone();
    let block = faces(&damaged);
    for face in damaged[block].as_chunks_mut::<FACE>().0.iter_mut().take(8) {
        face[..4].copy_from_slice(&0x7FFF_FFFF_u32.to_le_bytes());
    }

    let (intact, damaged) = (read(intact), read(damaged));
    assert!(damaged.triangle_count() < intact.triangle_count());
    assert!(
        damaged
            .warnings
            .iter()
            .any(|warning| matches!(warning, Warning::Repaired(_))),
        "{:?}",
        damaged.warnings
    );
}

const LARGE_FILE_HEADER: usize = 17;
const LARGE_BLOCK_HEADER: usize = 32;

struct Block {
    code: [u8; 4],
    address: u64,
    start: usize,
    length: usize,
}

fn blocks(bytes: &[u8]) -> Vec<Block> {
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
    let mut found = Vec::new();
    let mut at = LARGE_FILE_HEADER;
    while at + LARGE_BLOCK_HEADER <= bytes.len() {
        let code: [u8; 4] = bytes[at..at + 4].try_into().expect("four bytes");
        let length = word(at + 16) as usize;
        let start = at + LARGE_BLOCK_HEADER;
        found.push(Block {
            code,
            address: word(at + 8),
            start,
            length,
        });
        if &code == b"ENDB" {
            break;
        }
        at = start + length;
    }
    found
}

fn written_pair() -> Vec<u8> {
    let mut scene = Scene::new("Pair");
    let root = scene.scene_collection();
    let first = scene
        .add_object(
            Object::new("First", ObjectData::Empty(Empty::default())),
            root,
        )
        .expect("added");
    let second = scene
        .add_object(
            Object::new("Second", ObjectData::Empty(Empty::default())),
            root,
        )
        .expect("added");
    scene.object_mut(first).expect("exists").parent = Some(Parent::Object(second));
    write_scene(&scene, &BlendOptions::new())
        .expect("the scene is written")
        .bytes
}

#[test]
fn damaged_scene_files_fail() {
    let written = written_pair();
    let mut wrong_dna = written.clone();
    let dna = blocks(&written)
        .into_iter()
        .find(|block| &block.code == b"DNA1")
        .expect("the file has a DNA1 block");
    wrong_dna[dna.start..dna.start + 4].copy_from_slice(b"XXXX");
    let studio = std::fs::read(asset("studio.blend")).expect("the asset exists");
    let cases: [&[u8]; 4] = [
        &written[..written.len() / 2],
        &wrong_dna,
        &studio[..studio.len() / 3],
        b"BLENDER17-01v0502",
    ];
    for bytes in cases {
        assert!(
            scene_from("damaged.blend", bytes).is_err(),
            "{} bytes were accepted",
            bytes.len()
        );
    }
}

#[test]
fn cyclic_parents_are_broken() {
    let mut bytes = written_pair();
    let objects: Vec<Block> = blocks(&bytes)
        .into_iter()
        .filter(|block| &block.code == b"OB\0\0")
        .collect();
    let named = |name: &[u8]| {
        objects
            .iter()
            .find(|block| {
                bytes[block.start..block.start + block.length]
                    .windows(name.len())
                    .any(|window| window == name)
            })
            .expect("the object block exists")
    };
    let (first, second) = (named(b"OBFirst\0"), named(b"OBSecond\0"));
    let parent = bytes[first.start..first.start + first.length]
        .windows(8)
        .position(|window| window == second.address.to_le_bytes())
        .expect("the parent pointer is written");
    let at = second.start + parent;
    bytes[at..at + 8].copy_from_slice(&first.address.to_le_bytes());

    let read = scene_from("cyclic.blend", &bytes).expect("the file is read");
    read.scene.validate().expect("the scene is valid");
    assert!(
        read.warnings
            .iter()
            .any(|warning| matches!(warning, Warning::Repaired(_))),
        "{:?}",
        read.warnings
    );
}
