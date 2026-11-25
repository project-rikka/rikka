mod content;
mod instances;
mod worlds;

use std::io::Write;
use std::path::Path;

pub(crate) fn jar(path: &Path, manifest: &str) {
    let file = std::fs::File::create(path).expect("jar is created");
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .expect("entry starts");
    archive
        .write_all(manifest.as_bytes())
        .expect("manifest is written");
    archive.finish().expect("jar is finished");
}
