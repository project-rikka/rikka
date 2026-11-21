use std::sync::Arc;

use rikka_blend::BlendImporter;
use rikka_model_core::{Confidence, Import, Importer, Warning};

use crate::support::{asset, registry};

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
    let studio = std::fs::read(asset("studio.blend")).expect("the asset exists");
    let cases: [&[u8]; 5] = [
        b"BLENDER",
        b"BLENDER-v250REND",
        b"BLENDER17-01v0502",
        &unpacked[..unpacked.len() / 2],
        &studio[..studio.len() / 3],
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
