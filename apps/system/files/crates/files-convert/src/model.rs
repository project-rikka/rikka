use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use rikka_model::{
    DirectorySink, Export, ExportOptions, Import, MemorySink, Progress, Sink, registry,
};
use rikka_types::{CancelToken, Flow};

use crate::error::ConvertError;
use crate::landing::Landing;
use crate::progress::Watch;
use crate::target::ModelFormat;

const READ_SHARE: f32 = 0.6;

pub(crate) fn convert_model(
    source: &Path,
    format: ModelFormat,
    folder: &Path,
    name: &str,
    watch: &Watch,
) -> Result<PathBuf, ConvertError> {
    let cancel = CancelToken::new();
    let model = Import::open(source, registry())
        .cancel(cancel.clone())
        .progress(reporting(watch, &cancel, 0.0, READ_SHARE))
        .run()?;
    let resources = Arc::new(MemorySink::new());
    let exported = Export::new(&model, registry())
        .sink(Arc::clone(&resources) as Arc<dyn Sink>)
        .options(ExportOptions::new().embed_resources(format != ModelFormat::Gltf))
        .cancel(cancel.clone())
        .progress(reporting(watch, &cancel, READ_SHARE, 1.0))
        .to_bytes(name)?;
    for warning in &exported.warnings {
        tracing::info!(%warning, source = %source.display(), %name, "model conversion warning");
    }
    if watch.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    if resources.is_empty() {
        let landing = Landing::file(folder, name);
        written(landing.draft(), &exported.bytes)?;
        return Ok(landing.settle()?);
    }
    let landing = Landing::directory(folder, rikka_format::name_stem(name))?;
    written(&landing.draft().join(name), &exported.bytes)?;
    let sink = DirectorySink::new(landing.draft());
    for uri in resources.names() {
        if let Some(bytes) = resources.get(&uri) {
            sink.write(&uri, &bytes)?;
        }
    }
    Ok(landing.settle()?)
}

fn reporting(
    watch: &Watch,
    cancel: &CancelToken,
    from: f32,
    to: f32,
) -> impl Fn(Progress) + Send + Sync + 'static {
    let watch = watch.clone();
    let cancel = cancel.clone();
    let furthest = AtomicU32::new(from.to_bits());
    move |progress: Progress| {
        let reached = from + (to - from) * progress.fraction;
        let shown = f32::from_bits(furthest.fetch_max(reached.to_bits(), Ordering::Relaxed));
        if watch.reached(shown.max(reached)) == Flow::Cancel {
            cancel.cancel();
        }
    }
}

fn written(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
