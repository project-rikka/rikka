use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::catalog::classify::Classifier;
use crate::catalog::config::{CatalogConfig, Source};
use crate::catalog::scan::{ScannedFace, scan_bytes, scan_files};
use crate::cjk::CjkRegion;
use crate::config::Fontconfig;
use crate::coverage::{Coverage, PageTable};
use crate::discovery::{FileStamp, font_files};
use crate::face::{Face, FaceRecord, FaceSource};
use crate::family::fold;
use crate::index::{IndexContents, IndexStore, IndexedFile};
use crate::report::{LoadProblem, LoadReport};
use crate::system::{self, EmbeddedFont, SystemFonts};

pub(crate) struct Loaded {
    pub(crate) faces: Vec<Face>,
    pub(crate) fontconfig: Fontconfig,
    pub(crate) report: LoadReport,
}

struct LoadedFile {
    path: PathBuf,
    stamp: FileStamp,
    faces: Vec<FaceRecord>,
    readable: bool,
}

struct LoadedEmbedded {
    bytes: &'static [u8],
    faces: Vec<FaceRecord>,
}

pub(crate) fn load(config: &CatalogConfig) -> Loaded {
    let mut report = LoadReport::default();
    let SystemFonts {
        fontconfig,
        files,
        embedded,
    } = match &config.source {
        Source::Listed => SystemFonts::default(),
        Source::Fontconfig(file) => SystemFonts {
            fontconfig: Fontconfig::load(file, &config.environment, &mut report.problems),
            ..SystemFonts::default()
        },
        Source::System => system::fonts(&config.environment, &mut report.problems),
    };
    let dirs: Vec<PathBuf> = fontconfig
        .dirs
        .iter()
        .chain(&config.font_dirs)
        .cloned()
        .collect();
    let found = font_files(&files, &dirs, &fontconfig);
    let store = config.cache_dir.as_deref().map(IndexStore::new);
    let index = store
        .as_ref()
        .and_then(IndexStore::load)
        .unwrap_or_default();
    let mut pages = PageTable::from_pages(index.pages);
    let embedded = load_embedded(&embedded, &mut pages, &mut report.problems);
    let mut indexed: HashMap<PathBuf, IndexedFile> = index
        .files
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect();
    let mut files = Vec::with_capacity(found.len());
    let mut stale = Vec::new();
    for font in found {
        match indexed.remove(&font.path) {
            Some(entry) if entry.stamp.same_content(&font.stamp) => files.push(LoadedFile {
                path: font.path,
                stamp: entry.stamp,
                faces: entry.faces,
                readable: true,
            }),
            _ => {
                stale.push(files.len());
                files.push(LoadedFile {
                    path: font.path,
                    stamp: font.stamp,
                    faces: Vec::new(),
                    readable: true,
                });
            }
        }
    }
    report.indexed_files = files.len() - stale.len();
    report.scanned_files = stale.len();
    rescan(&mut files, &stale, &mut pages, &mut report.problems);
    if let Some(store) = store.filter(|_| !stale.is_empty()) {
        let leftovers = indexed.into_values().filter(|file| file.path.exists());
        let contents = index_contents(&files, leftovers, &pages);
        if let Err(source) = store.save(&contents) {
            report.problems.push(LoadProblem::IndexWrite {
                path: store.path().to_owned(),
                source,
            });
        }
    }
    Loaded {
        faces: assemble(embedded, files, pages, &fontconfig),
        fontconfig,
        report,
    }
}

fn load_embedded(
    fonts: &[EmbeddedFont],
    pages: &mut PageTable,
    problems: &mut Vec<LoadProblem>,
) -> Vec<LoadedEmbedded> {
    let sources: Vec<(&[u8], String)> = fonts
        .iter()
        .map(|font| (font.bytes, font.stem().to_owned()))
        .collect();
    fonts
        .iter()
        .zip(scan_bytes(&sources))
        .map(|(font, scanned)| {
            if scanned.is_empty() {
                problems.push(LoadProblem::UnusableEmbeddedFont(font.name));
            }
            LoadedEmbedded {
                bytes: font.bytes,
                faces: intern(scanned, pages),
            }
        })
        .collect()
}

fn intern(scanned: Vec<ScannedFace>, pages: &mut PageTable) -> Vec<FaceRecord> {
    scanned
        .into_iter()
        .map(|(mut record, coverage)| {
            record.coverage = coverage
                .into_iter()
                .map(|(number, page)| (number, pages.intern(page)))
                .collect();
            record
        })
        .collect()
}

fn rescan(
    files: &mut [LoadedFile],
    stale: &[usize],
    pages: &mut PageTable,
    problems: &mut Vec<LoadProblem>,
) {
    let paths: Vec<&Path> = stale
        .iter()
        .map(|&position| files[position].path.as_path())
        .collect();
    let scanned = scan_files(&paths);
    for (&position, outcome) in stale.iter().zip(scanned) {
        let file = &mut files[position];
        match outcome {
            Ok(scanned) => {
                file.stamp = scanned.stamp;
                file.faces = intern(scanned.faces, pages);
                if file.faces.is_empty() {
                    problems.push(LoadProblem::UnusableFont(file.path.clone()));
                }
            }
            Err(source) => {
                file.readable = false;
                problems.push(LoadProblem::UnreadableFont {
                    path: file.path.clone(),
                    source,
                });
            }
        }
    }
}

fn index_contents(
    files: &[LoadedFile],
    leftovers: impl Iterator<Item = IndexedFile>,
    pages: &PageTable,
) -> IndexContents {
    let mut table = PageTable::default();
    let mut remap = |records: &[FaceRecord]| -> Vec<FaceRecord> {
        records
            .iter()
            .map(|record| FaceRecord {
                coverage: record
                    .coverage
                    .iter()
                    .filter_map(|&(number, slot)| Some((number, table.intern(*pages.page(slot)?))))
                    .collect(),
                ..record.clone()
            })
            .collect()
    };
    let mut indexed: Vec<IndexedFile> = files
        .iter()
        .filter(|file| file.readable)
        .map(|file| IndexedFile {
            path: file.path.clone(),
            stamp: file.stamp,
            faces: remap(&file.faces),
        })
        .collect();
    for leftover in leftovers {
        indexed.push(IndexedFile {
            faces: remap(&leftover.faces),
            ..leftover
        });
    }
    IndexContents {
        pages: table.into_pages(),
        files: indexed,
    }
}

fn assemble(
    embedded: Vec<LoadedEmbedded>,
    files: Vec<LoadedFile>,
    pages: PageTable,
    fontconfig: &Fontconfig,
) -> Vec<Face> {
    let store = pages.into_store();
    let classifier = Classifier::new(&fontconfig.aliases);
    let sourced = embedded
        .into_iter()
        .map(|font| (FaceSource::Embedded(font.bytes), font.faces))
        .chain(files.into_iter().map(|file| {
            let source = FaceSource::File {
                path: Arc::from(file.path),
                stamp: file.stamp,
            };
            (source, file.faces)
        }));
    let mut faces = Vec::new();
    for (source, records) in sourced {
        for mut record in records {
            let coverage = Coverage::new(&std::mem::take(&mut record.coverage), store.clone());
            let folded_families: Box<[String]> =
                record.families.iter().map(|name| fold(name)).collect();
            let class = classifier.classify(&record, &folded_families);
            let cjk_region = folded_families
                .iter()
                .find_map(|family| CjkRegion::from_family(family));
            faces.push(Face {
                source: source.clone(),
                record,
                coverage,
                folded_families,
                class,
                cjk_region,
            });
        }
    }
    faces
}
