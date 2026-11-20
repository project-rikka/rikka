use std::path::Path;

use files_convert::{Request, Target, Tuning, convert};
use files_vfs::Outcome;
use rikka_image::ColorScheme;
use rikka_mime::MimeType;

pub(crate) fn mime(essence: &str) -> MimeType {
    MimeType::parse(essence).expect("valid media type")
}

pub(crate) fn converted(sources: &[&Path], target: Target, suffix: &str) -> Outcome {
    converted_in(sources, target, ColorScheme::Light, suffix)
}

pub(crate) fn converted_in(
    sources: &[&Path],
    target: Target,
    scheme: ColorScheme,
    suffix: &str,
) -> Outcome {
    let job = convert(Request {
        sources: sources.iter().map(|source| source.to_path_buf()).collect(),
        target,
        tuning: Tuning::suited(target, scheme),
        suffix: suffix.to_owned(),
    });
    job.wait()
}

pub(crate) fn succeeded(sources: &[&Path], target: Target, suffix: &str) {
    let outcome = converted(sources, target, suffix);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!outcome.cancelled);
}

pub(crate) fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("read output directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
