use rikka_model_core::{Error, Import};

use crate::support::{binary, registry};

fn refused(bytes: Vec<u8>) -> String {
    match Import::named_bytes("broken.stl", bytes, registry()).run() {
        Err(Error::Damaged(reason)) => reason,
        other => panic!("a damaged file was not refused: {other:?}"),
    }
}

#[test]
fn corrupt_files_are_rejected() {
    let mut cut = binary(b"cut short", &[([[0.0; 3]; 4], 0)]);
    cut[80..84].copy_from_slice(&10u32.to_le_bytes());
    assert!(refused(cut).contains("10 triangles"));

    let stray = b"solid broken\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   \
                  vertex 1 0 0\n   vertx 0 1 0\n  endloop\n endfacet\nendsolid broken\n";
    assert!(refused(stray.to_vec()).contains("line 6"));

    let endless = b"solid broken\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   \
                    vertex 1 0 0\n   vertex 0 inf 0\n  endloop\n endfacet\nendsolid broken\n";
    assert!(refused(endless.to_vec()).contains("\"inf\""));
}
