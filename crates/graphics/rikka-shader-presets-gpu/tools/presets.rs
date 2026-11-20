use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rikka_shader_graph::Library;
use rikka_shader_programs::{TemplateManifest, Variant, compile_template, source};
use rikka_slang::{Compiler, Optimization};
use rikka_spirv::Facts;

type Failure = Box<dyn std::error::Error>;

const USAGE: &str = "usage:
  presets template --key KEY --output DIRECTORY [--module NAME=PATH]...
  presets listing --output FILE [--preset KEY=INCLUDED=DIRECTORY]...";

const MANIFEST: &str = "manifest.json";

const SPIRV_EXTENSION: &str = "spv";

#[derive(Default)]
struct Arguments {
    key: Option<String>,
    output: Option<PathBuf>,
    modules: Vec<(String, PathBuf)>,
    presets: Vec<(String, String, PathBuf)>,
}

fn pair(argument: &str, shape: &str) -> Result<(String, String), Failure> {
    let (left, right) = argument
        .split_once('=')
        .ok_or_else(|| format!("expected {shape}, got {argument}"))?;
    Ok((left.to_owned(), right.to_owned()))
}

fn parsed(mut rest: impl Iterator<Item = String>) -> Result<Arguments, Failure> {
    let mut arguments = Arguments::default();
    while let Some(argument) = rest.next() {
        let mut value = || {
            rest.next()
                .ok_or_else(|| format!("{argument} needs a value\n{USAGE}"))
        };
        match argument.as_str() {
            "--key" => arguments.key = Some(value()?),
            "--output" => arguments.output = Some(value()?.into()),
            "--module" => {
                let (name, path) = pair(&value()?, "NAME=PATH")?;
                arguments.modules.push((name, path.into()));
            }
            "--preset" => {
                let (key, location) = pair(&value()?, "KEY=INCLUDED=DIRECTORY")?;
                let (included, directory) = pair(&location, "KEY=INCLUDED=DIRECTORY")?;
                arguments.presets.push((key, included, directory.into()));
            }
            _ => return Err(format!("unexpected {argument}\n{USAGE}").into()),
        }
    }
    Ok(arguments)
}

fn required<T>(value: Option<T>, flag: &str) -> Result<T, Failure> {
    value.ok_or_else(|| format!("{flag} is required\n{USAGE}").into())
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Failure> {
    std::fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()).into())
}

fn compiler(
    modules: &[(String, PathBuf)],
    optimization: Optimization,
) -> Result<Compiler, Failure> {
    let mut builder = Compiler::builder().optimization(optimization);
    for (name, path) in modules {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        builder = builder.module(name, text);
    }
    Ok(builder.build()?)
}

fn template(arguments: Arguments) -> Result<(), Failure> {
    let key = required(arguments.key, "--key")?;
    let output = required(arguments.output, "--output")?;
    let preset = rikka_shader_presets::preset(&key).ok_or_else(|| format!("no preset {key}"))?;
    let library = Library::standard();
    let material = preset
        .graph(&library)
        .and_then(|graph| graph.material(&library, &key))
        .map_err(|error| format!("{key}: {error}"))?;
    let compiled = compile_template(
        &compiler(&arguments.modules, Optimization::High)?,
        Some(&compiler(&arguments.modules, Optimization::None)?),
        &key,
        &source(&material.source),
    )
    .map_err(|error| format!("{key}: {error}\n{}", material.source))?;
    std::fs::create_dir_all(&output)?;
    for (variant, words) in Variant::ALL.iter().zip(&compiled.variants) {
        write(
            &output.join(format!("{}.{SPIRV_EXTENSION}", variant.entry())),
            rikka_spirv::bytes(words),
        )?;
    }
    let manifest = TemplateManifest {
        schema: compiled.schema,
        deforms: compiled.deforms,
        scatters: compiled.scatters,
    };
    write(&output.join(MANIFEST), serde_json::to_string(&manifest)?)
}

fn shader(directory: &Path, included: &str, entry: &str) -> Result<String, Failure> {
    let file = format!("{entry}.{SPIRV_EXTENSION}");
    let path = directory.join(&file);
    let spirv = std::fs::read(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let facts = Facts::of(&spirv).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(facts.declaration(&format!("{included}/{file}")))
}

fn listing(arguments: Arguments) -> Result<(), Failure> {
    let output = required(arguments.output, "--output")?;
    let mut text = String::from(
        "pub static PRESETS: &[(&str, rikka_shader_programs::TemplateArtifact)] = &[\n",
    );
    let mut presets = arguments.presets;
    presets.sort();
    for (key, included, directory) in &presets {
        let variants = Variant::ALL
            .iter()
            .map(|variant| shader(directory, included, variant.entry()))
            .collect::<Result<Vec<_>, Failure>>()?
            .join(", ");
        writeln!(
            text,
            "    ({key:?}, rikka_shader_programs::TemplateArtifact {{ manifest: include_str!(\"{included}/{MANIFEST}\"), variants: [{variants}] }}),"
        )?;
    }
    text.push_str("];\n");
    write(&output, text)
}

fn run() -> Result<(), Failure> {
    let mut rest = std::env::args().skip(1);
    let task = rest.next();
    let arguments = parsed(rest)?;
    match task.as_deref() {
        Some("template") => template(arguments),
        Some("listing") => listing(arguments),
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
