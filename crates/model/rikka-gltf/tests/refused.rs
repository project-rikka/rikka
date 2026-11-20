use rikka_model_core::{Error, Import, ImportOptions, Limit, Limits, Model, Result};

use crate::support::registry;

fn read(bytes: Vec<u8>) -> Result<Model> {
    Import::named_bytes("model.glb", bytes, registry()).run()
}

fn binary(version: u32, chunk_length: u32, json: &[u8]) -> Vec<u8> {
    let length = 12 + 8 + json.len() as u32;
    let mut bytes = b"glTF".to_vec();
    bytes.extend(version.to_le_bytes());
    bytes.extend(length.to_le_bytes());
    bytes.extend(chunk_length.to_le_bytes());
    bytes.extend(b"JSON");
    bytes.extend(json);
    bytes
}

const EMPTY: &[u8] = br#"{"asset":{"version":"2.0","generator":"a tool"}}"#;

#[test]
fn empty_document_is_empty_model() {
    for bytes in [EMPTY.to_vec(), binary(2, EMPTY.len() as u32, EMPTY)] {
        let model = read(bytes).expect("an empty model");
        assert_eq!(model.metadata.generator.as_deref(), Some("a tool"));
        assert!(model.nodes.is_empty());
    }
}

#[test]
fn unsupported_document_is_rejected() {
    let unsupported = [
        br#"{"asset":{"version":"1.0"}}"#.to_vec(),
        br#"{"asset":{"version":"2.0"},"extensionsRequired":["KHR_draco_mesh_compression"]}"#
            .to_vec(),
        binary(3, EMPTY.len() as u32, EMPTY),
    ];
    for bytes in unsupported {
        let error = read(bytes).expect_err("an unsupported document");
        assert!(
            matches!(
                error,
                Error::UnsupportedVersion { .. } | Error::RequiredExtensions(_)
            ),
            "{error}"
        );
    }

    let damaged = [
        b"{ this is not json".to_vec(),
        binary(2, 0xFFFF, EMPTY),
        binary(2, EMPTY.len() as u32, EMPTY)[..14].to_vec(),
    ];
    for bytes in damaged {
        let error = read(bytes).expect_err("a damaged document");
        assert!(matches!(error, Error::Damaged(_)), "{error}");
    }
}

#[test]
fn out_of_bounds_accessor_is_rejected() {
    let document = r#"{
      "asset": { "version": "2.0" },
      "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA", "byteLength": 36 }],
      "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
      "accessors": [
        { "bufferView": 0, "componentType": 5126, "count": 9, "type": "VEC3",
          "min": [0, 0, 0], "max": [1, 1, 0] }
      ],
      "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
      "nodes": [{ "mesh": 0 }],
      "scenes": [{ "nodes": [0] }]
    }"#;

    let error = Import::named_bytes("model.gltf", document.as_bytes().to_vec(), registry())
        .run()
        .expect_err("an accessor past its buffer");

    assert!(matches!(error, Error::Damaged(_)), "{error}");
}

#[test]
fn declared_counts_are_bounded_before_allocation() {
    let unbacked = |count: &str| {
        format!(
            r#"{{
              "asset": {{ "version": "2.0" }},
              "accessors": [{{ "componentType": 5126, "count": {count}, "type": "VEC3" }}],
              "meshes": [{{ "primitives": [{{ "attributes": {{ "POSITION": 0 }} }}] }}]
            }}"#
        )
    };
    let strided = r#"{
      "asset": { "version": "2.0" },
      "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAA", "byteLength": 12 }],
      "bufferViews": [{ "buffer": 0, "byteLength": 12, "byteStride": 8 }],
      "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 2305843009213693953, "type": "SCALAR" }],
      "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 0 }] }]
    }"#;
    let refused = |document: &str| {
        Import::named_bytes("model.gltf", document.as_bytes().to_vec(), registry())
            .run()
            .expect_err("a count no file can back")
    };

    let error = refused(&unbacked("3000000000"));
    assert!(matches!(error, Error::LimitsExceeded { .. }), "{error}");
    for document in [unbacked("4611686018427387904"), strided.to_owned()] {
        let error = refused(&document);
        assert!(matches!(error, Error::Damaged(_)), "{error}");
    }
}

#[test]
fn repeated_streams_stop_at_the_vertex_limit() {
    let document = r#"{
      "asset": { "version": "2.0" },
      "accessors": [{ "componentType": 5126, "count": 60, "type": "VEC3" }],
      "meshes": [{ "primitives": [
        { "attributes": { "POSITION": 0 } },
        { "attributes": { "POSITION": 0, "_FIRST": 0 } },
        { "attributes": { "POSITION": 0, "_SECOND": 0 } }
      ] }]
    }"#;
    let limits = Limits {
        max_vertices: 100,
        ..Limits::default()
    };
    let error = Import::named_bytes("model.gltf", document.as_bytes().to_vec(), registry())
        .options(ImportOptions::new().limits(limits))
        .run()
        .expect_err("more vertices than allowed");
    assert!(
        matches!(
            error,
            Error::LimitsExceeded {
                limit: Limit::Vertices,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn keyframe_time_that_is_not_a_number_is_rejected() {
    let document = r#"{
      "asset": { "version": "2.0" },
      "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAAAAAAAAAAAAAA==", "byteLength": 16 }],
      "bufferViews": [
        { "buffer": 0, "byteOffset": 0, "byteLength": 4 },
        { "buffer": 0, "byteOffset": 4, "byteLength": 12 }
      ],
      "accessors": [
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "SCALAR" },
        { "bufferView": 1, "componentType": 5126, "count": 1, "type": "VEC3" }
      ],
      "nodes": [{ "name": "moved" }],
      "scenes": [{ "nodes": [0] }],
      "animations": [{
        "samplers": [{ "input": 0, "output": 1 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }]
      }]
    }"#;
    let error = Import::named_bytes("model.gltf", document.as_bytes().to_vec(), registry())
        .run()
        .expect_err("a keyframe at no time");
    assert!(matches!(error, Error::Invalid(_)), "{error}");
}
