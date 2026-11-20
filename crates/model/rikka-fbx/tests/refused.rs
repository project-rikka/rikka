use rikka_model_core::{Error, Import};

use crate::support::{KNIGHT, registry};

const HEADER: usize = 27;

#[test]
fn truncated_document_is_rejected() {
    let mut reaching = KNIGHT.to_vec();
    reaching[HEADER..HEADER + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let cut = KNIGHT[..KNIGHT.len() / 2].to_vec();
    let open = b"; FBX 7.4.0 project file\nFBXHeaderExtension:  {\n\tFBXVersion: 7400\n".to_vec();

    for (what, bytes) in [
        ("a record reaching past the document", reaching),
        ("a document cut in half", cut),
        ("a text record left open", open),
    ] {
        let error = Import::named_bytes("damaged.fbx", bytes, registry())
            .run()
            .expect_err(what);
        assert!(matches!(error, Error::Damaged(_)), "{what}: {error}");
    }
}
