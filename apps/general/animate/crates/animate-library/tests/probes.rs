use std::path::Path;

use animate_library::{Details, Format};

use crate::support::{AVATAR, CLIP_ONLY, DANCE, MIKU, glb, stl, vmd};

fn read(bytes: &[u8], name: &str) -> Details {
    let folder = tempfile::tempdir().expect("temporary folder");
    let path = folder.path().join(name);
    std::fs::write(&path, bytes).expect("file written");
    Details::read(
        &path,
        Format::from_path(Path::new(name)).expect("known format"),
    )
}

#[test]
fn pmx_header_gives_title_and_vertices() {
    let Details::Model(model) = read(MIKU, "miku.pmx") else {
        panic!("pmx reads as a model");
    };
    assert!(model.title.is_some());
    assert!(model.vertices.is_some_and(|vertices| vertices > 0));
}

#[test]
fn vmd_counts_keys_and_last_frame() {
    let Details::Motion(motion) = read(&vmd(3, 2, 0, 450), "take.vmd") else {
        panic!("vmd reads as a motion");
    };
    assert_eq!(motion.frames, Some(450));
    assert!((motion.seconds - 15.0).abs() < 1e-4);
    assert!(motion.body && motion.morphs && !motion.camera);
    assert_eq!(motion.target.as_deref(), Some("model"));

    let Details::Motion(dance) = read(DANCE, "dance.vmd") else {
        panic!("vmd reads as a motion");
    };
    assert!(dance.body && dance.frames.is_some_and(|frames| frames > 0));
}

#[test]
fn gltf_clips_carry_their_length() {
    let Details::Model(model) = read(&glb(CLIP_ONLY), "clip.glb") else {
        panic!("glb reads as a model");
    };
    assert_eq!(model.meshes, Some(0));
    assert_eq!(model.clips.len(), 1);
    assert_eq!(model.clips[0].name, "wave");
    assert!((model.clips[0].seconds - 1.5).abs() < 1e-6);
}

#[test]
fn vrm_meta_and_thumbnail_are_read() {
    let details = read(AVATAR, "avatar.vrm");
    assert!(details.has_embedded_picture());
    let Details::Model(model) = details else {
        panic!("vrm reads as a model");
    };
    assert!(
        model
            .vrm
            .is_some_and(|vrm| !vrm.meta.license_url.is_empty())
    );
    assert!(model.meshes.is_some_and(|meshes| meshes > 0));
}

#[test]
fn stl_counts_triangles() {
    let Details::Model(model) = read(&stl(4), "part.stl") else {
        panic!("stl reads as a model");
    };
    assert_eq!(model.vertices, Some(12));
}

#[test]
fn malformed_files_are_unreadable() {
    let mut huge_vmd = vmd(0, 0, 0, 0);
    huge_vmd[50..54].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut huge_pmx = MIKU[..17].to_vec();
    huge_pmx.extend_from_slice(&i32::MAX.to_le_bytes());
    let mut huge_glb = glb(CLIP_ONLY);
    huge_glb[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    let cases: [(&str, &[u8]); 19] = [
        ("empty.pmx", b""),
        ("garbage.pmx", b"not a model at all"),
        ("short.pmx", &MIKU[..100]),
        ("huge.pmx", &huge_pmx),
        ("empty.vmd", b""),
        ("short.vmd", &DANCE[..1000]),
        ("huge.vmd", &huge_vmd),
        ("empty.vrm", b""),
        ("short.vrm", &AVATAR[..5000]),
        ("huge.glb", &huge_glb),
        ("broken.gltf", b"{\"meshes\": 4}"),
        ("plain.vrm", &glb(CLIP_ONLY)),
        ("plain.vrma", &glb(CLIP_ONLY)),
        ("garbage.blend", b"BLEN"),
        ("garbage.stl", b"\x01\x02\x03"),
        ("garbage.rmotion", b"{\"format\": \"rikka-motion\""),
        ("garbage.wav", b"RIFF\x04\x00\x00\x00WAVE"),
        ("text.wav", b"plain text"),
        ("empty.mp3", b""),
    ];
    for (name, bytes) in cases {
        assert!(
            matches!(read(bytes, name), Details::Unreadable(_)),
            "{name} reads as unreadable"
        );
    }
}
