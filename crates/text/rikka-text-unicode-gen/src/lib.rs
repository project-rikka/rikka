mod aliases;
mod dictionary;
mod emit;
mod enumeration;
mod error;
mod line_break;
mod model;
mod rust;
mod source;
mod trie;
mod ucd;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use emit::Report;
use error::{Error, Result};
use model::Model;
use source::Directory;

const CRATE: &str = "crates/text/rikka-text-unicode";
const USAGE: &str = "usage: rikka-text-unicode-gen <repository root>";

pub fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let (Some(root), None) = (arguments.next(), arguments.next()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match regenerate(Path::new(&root)) {
        Ok(report) => {
            for (table, bytes) in &report.sizes {
                println!("{table}: {bytes} bytes");
            }
            let total: usize = report.sizes.iter().map(|(_, bytes)| bytes).sum();
            println!("total: {total} bytes");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("rikka-text-unicode-gen: {error}");
            ExitCode::FAILURE
        }
    }
}

fn regenerate(root: &Path) -> Result<Report> {
    let crate_root = root.join(CRATE);
    let report = generate(
        &Directory::new(crate_root.join("ucd")),
        &Directory::new(crate_root.join("dictionaries")),
    )?;
    let tables: PathBuf = crate_root.join("src/tables");
    fs::create_dir_all(&tables).map_err(|source| Error::Write {
        path: tables.clone(),
        source,
    })?;
    for file in &report.files {
        let path = tables.join(&file.name);
        fs::write(&path, &file.contents).map_err(|source| Error::Write { path, source })?;
    }
    Ok(report)
}

fn generate(ucd: &Directory, dictionaries: &Directory) -> Result<Report> {
    let version = version(&ucd.read("DerivedCoreProperties.txt")?)?;
    emit::generate(
        &Model::load(ucd)?,
        &dictionary::compile(dictionaries)?,
        version,
    )
}

fn version(header: &str) -> Result<(u32, u32, u32)> {
    let invalid = || Error::Data("DerivedCoreProperties.txt has no version header".into());
    let name = header.lines().next().ok_or_else(invalid)?;
    let version = name
        .strip_prefix("# DerivedCoreProperties-")
        .and_then(|rest| rest.strip_suffix(".txt"))
        .ok_or_else(invalid)?;
    let mut parts = version.split('.').map(str::parse::<u32>);
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor)), Some(Ok(update)), None) => Ok((major, minor, update)),
        _ => Err(invalid()),
    }
}
