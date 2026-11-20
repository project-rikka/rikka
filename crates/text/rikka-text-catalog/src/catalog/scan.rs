use std::io;
use std::path::Path;

use rikka_text_font::FontFile;

use crate::coverage::{CoverageBuilder, Page};
use crate::discovery::{FileStamp, Mapping};
use crate::face::{FaceRecord, extract};
use crate::parallel::parallel_map;

pub(crate) type ScannedFace = (FaceRecord, Vec<(u16, Page)>);

pub(crate) struct ScannedFile {
    pub(crate) stamp: FileStamp,
    pub(crate) faces: Vec<ScannedFace>,
}

pub(crate) fn scan_files(paths: &[&Path]) -> Vec<io::Result<ScannedFile>> {
    let mapped = parallel_map(paths, || (), |(), path| Mapping::open(path));
    let sources: Vec<(&[u8], String)> = mapped
        .iter()
        .zip(paths)
        .map(|(mapping, path)| {
            let bytes = mapping
                .as_ref()
                .map_or(&[][..], |(mapping, _)| mapping.bytes());
            (bytes, stem(path))
        })
        .collect();
    let scanned = scan_bytes(&sources);
    mapped
        .into_iter()
        .zip(scanned)
        .map(|(mapping, faces)| {
            let (_, stamp) = mapping?;
            Ok(ScannedFile { stamp, faces })
        })
        .collect()
}

pub(crate) fn scan_bytes(sources: &[(&[u8], String)]) -> Vec<Vec<ScannedFace>> {
    let mut jobs: Vec<(usize, u32)> = Vec::new();
    for (file, (bytes, _)) in sources.iter().enumerate() {
        if let Ok(font_file) = FontFile::new(bytes) {
            jobs.extend((0..font_file.face_count()).map(|face| (file, face)));
        }
    }
    jobs.sort_by_key(|&(file, _)| std::cmp::Reverse(sources[file].0.len()));
    let faces = parallel_map(&jobs, CoverageBuilder::new, |coverage, &(file, face)| {
        let (bytes, stem) = &sources[file];
        let font = FontFile::new(bytes).ok()?.face(face).ok()?;
        Some(extract(&font, stem, coverage))
    });
    let mut grouped: Vec<Vec<(u32, ScannedFace)>> = sources.iter().map(|_| Vec::new()).collect();
    for (&(file, index), face) in jobs.iter().zip(faces) {
        if let Some(face) = face {
            grouped[file].push((index, face));
        }
    }
    grouped
        .into_iter()
        .map(|mut faces| {
            faces.sort_by_key(|&(index, _)| index);
            faces.into_iter().map(|(_, face)| face).collect()
        })
        .collect()
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}
