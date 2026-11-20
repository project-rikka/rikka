use std::path::Path;
use std::sync::Arc;

use rikka_model_core::glam::Vec3;
use rikka_model_core::{
    Confidence, CoordinateSystem, Error, Export, ExportOptions, Import, ImportCtx, ImportOptions,
    Importer, ImporterInfo, Input, Limits, MemoryResolver, MemorySink, Model, Registry, Result,
    Sink, Supports,
};
use rikka_types::CancelToken;

use crate::support::{TEXTURE, keeping, model, registry, toy_options};

fn written_beside(folder: &Path) -> std::path::PathBuf {
    let path = folder.join("model.toy");
    Export::new(&model(), registry())
        .options(toy_options().embed_resources(false))
        .to_file(&path)
        .expect("a written model");
    path
}

#[test]
fn overwrite_keeps_permissions_and_symlinks() {
    use std::os::unix::fs::PermissionsExt;

    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = written_beside(folder.path());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).expect("a mode");
    let linked = folder.path().join("linked.toy");
    std::os::unix::fs::symlink(&path, &linked).expect("a link");

    Export::new(&model(), registry())
        .options(toy_options().embed_resources(false))
        .to_file(&linked)
        .expect("a model written through the link");

    let mode = std::fs::metadata(&path)
        .expect("the model")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
    assert!(
        std::fs::symlink_metadata(&linked)
            .expect("the link")
            .file_type()
            .is_symlink()
    );
    let leftovers: Vec<_> = std::fs::read_dir(folder.path())
        .expect("the folder")
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn foreign_coordinate_export_round_trips() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let original = model();

    let exported = Export::new(&original, registry())
        .options(toy_options())
        .to_bytes("model.toy")
        .expect("a written model");
    assert!(
        String::from_utf8_lossy(&exported.bytes).contains("v 0 20 -30"),
        "the document is in the toy's own system and units"
    );
    assert_eq!(
        original.meshes[0].vertices.positions[2],
        Vec3::new(0.0, 2.0, 3.0),
        "the model itself is not touched"
    );
    assert_eq!(original.metadata.coordinates, CoordinateSystem::CANONICAL);

    let read = Import::open(written_beside(folder.path()), registry())
        .run()
        .expect("the model read back");
    assert_eq!(read.metadata.coordinates, CoordinateSystem::CANONICAL);
    assert_eq!(read.metadata.unit_scale, 1.0);
    assert_eq!(
        read.meshes[0].vertices.positions,
        original.meshes[0].vertices.positions
    );
    assert_eq!(
        read.nodes[0].transform.translation,
        original.nodes[0].transform.translation
    );
}

#[test]
fn external_texture_round_trips_or_warns() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = written_beside(folder.path());
    assert!(folder.path().join("Textures/Skin.png").exists());

    let read = Import::open(&path, registry())
        .run()
        .expect("the model read back");
    assert_eq!(read.images.len(), 1);
    assert_eq!(read.images[0].bytes.as_ref(), b"pixels");
    assert_eq!(read.metadata.source.as_deref(), Some(path.as_path()));
    assert!(read.warnings.is_empty());

    std::fs::remove_file(folder.path().join("Textures/Skin.png")).expect("the texture goes");
    let read = Import::open(&path, registry())
        .run()
        .expect("the model read back without its texture");
    assert!(read.images.is_empty());
    assert_eq!(read.warnings.len(), 1, "{:?}", read.warnings);
}

#[test]
fn memory_resolver_serves_resources() {
    let document = Export::new(&model(), registry())
        .sink(Arc::new(MemorySink::new()))
        .options(ExportOptions::new().embed_resources(false))
        .to_bytes("model.toy")
        .expect("a written model")
        .bytes;
    let resolver = MemoryResolver::new().with_file(TEXTURE, b"pixels".to_vec());

    let read = Import::bytes(document, registry())
        .resolver(Arc::new(resolver))
        .run()
        .expect("a model with its texture");

    assert!(read.warnings.is_empty());
    assert_eq!(read.images[0].bytes.as_ref(), b"pixels");
}

#[test]
fn export_without_sink_rejects_resources() {
    let model = model();
    let sink = Arc::new(MemorySink::new());
    Export::new(&model, registry())
        .sink(Arc::clone(&sink) as Arc<dyn Sink>)
        .options(ExportOptions::new().embed_resources(false))
        .to_bytes("model.toy")
        .expect("a written model");
    assert_eq!(sink.get("textures/skin.png"), Some(b"pixels".to_vec()));

    let error = Export::new(&model, registry())
        .options(ExportOptions::new().embed_resources(false))
        .to_bytes("model.toy")
        .expect_err("nothing takes the texture");
    assert!(matches!(error, Error::Write { .. }), "{error:?}");
    assert!(error.to_string().starts_with("failed to write"), "{error}");
}

#[test]
fn export_warns_about_unsupported_data() {
    let exported = Export::new(&model(), keeping(Supports::all().skins(false)))
        .to_bytes("model.toy")
        .expect("a written model");

    assert_eq!(exported.warnings.len(), 1, "{:?}", exported.warnings);
}

#[test]
fn unknown_format_is_rejected() {
    let error = Import::bytes(b"nothing".to_vec(), registry())
        .run()
        .expect_err("no importer reads this");
    assert!(matches!(error, Error::UnknownFormat(_)), "{error:?}");

    let error = Export::new(&model(), registry())
        .to_bytes("model.unknown")
        .expect_err("no exporter writes this");
    assert!(matches!(error, Error::UnknownFormat(_)), "{error:?}");
}

#[test]
fn cancelled_import_and_export_do_nothing() {
    let cancel = CancelToken::new();
    cancel.cancel();

    let error = Import::bytes(b"TOY 1".to_vec(), registry())
        .cancel(cancel.clone())
        .run()
        .expect_err("a cancelled import");
    assert!(matches!(error, Error::Cancelled), "{error:?}");

    let error = Export::new(&model(), registry())
        .cancel(cancel)
        .to_bytes("model.toy")
        .expect_err("a cancelled export");
    assert!(matches!(error, Error::Cancelled), "{error:?}");
}

#[test]
fn import_over_limits_is_rejected() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = written_beside(folder.path());

    for limits in [
        Limits {
            max_file_bytes: 4,
            ..Limits::default()
        },
        Limits {
            max_vertices: 2,
            ..Limits::default()
        },
    ] {
        let error = Import::open(&path, registry())
            .options(ImportOptions::default().limits(limits))
            .run()
            .expect_err("a model over the limit");
        assert!(matches!(error, Error::LimitsExceeded { .. }), "{error:?}");
    }
}

struct Sniffing {
    name: &'static str,
    magic: Option<&'static [u8]>,
}

impl Importer for Sniffing {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: self.name,
            extensions: if self.magic.is_some() {
                &["magic"]
            } else {
                &["loose"]
            },
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        match self.magic {
            Some(magic) if head.starts_with(magic) => Confidence::Yes,
            Some(_) => Confidence::No,
            None => Confidence::Maybe,
        }
    }

    fn read(&self, _input: &Input<'_>, _ctx: &mut ImportCtx<'_>) -> Result<Model> {
        Ok(Model::default())
    }
}

#[test]
fn content_sniffing_beats_extension() {
    let magic = || Sniffing {
        name: "magic",
        magic: Some(b"MAGIC"),
    };
    let registry = Registry::new()
        .with_importer(Sniffing {
            name: "loose",
            magic: None,
        })
        .with_importer(magic());
    let detect = |bytes: &[u8], name: &str| {
        registry
            .importer_for(&Input::new(bytes, Some(Path::new(name))))
            .map(|importer| importer.info().name)
    };

    assert_eq!(detect(b"MAGIC and then some", "model.loose"), Some("magic"));
    assert_eq!(detect(b"nothing familiar", "model.LOOSE"), Some("loose"));
    assert!(
        Registry::new()
            .with_importer(magic())
            .importer_for(&Input::new(b"not magic", Some(Path::new("model.magic"))))
            .is_none(),
        "an importer that says no is never chosen for its own extension"
    );
}
