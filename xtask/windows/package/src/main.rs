mod archive;
mod entries;
mod imports;
mod system;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::entries::{Entries, Placement};

#[derive(Parser)]
#[command(about = "Packs Rikka's portable Windows distribution as a deterministic ZIP")]
struct Arguments {
    #[arg(long)]
    objdump: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long = "program", value_name = "EXE=ENTRY")]
    programs: Vec<Placement>,
    #[arg(long = "tree", value_name = "DIRECTORY=ENTRY")]
    trees: Vec<Placement>,
    #[arg(long = "file", value_name = "FILE=ENTRY")]
    files: Vec<Placement>,
}

fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let mut entries = Entries::default();
    for program in &arguments.programs {
        entries.program(program)?;
    }
    for tree in &arguments.trees {
        entries.tree(tree)?;
    }
    for file in &arguments.files {
        entries.file(file)?;
    }
    imports::check(&arguments.objdump, &entries)?;
    archive::write(&entries, &arguments.output)
}
