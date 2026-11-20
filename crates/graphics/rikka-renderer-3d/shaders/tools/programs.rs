use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rikka_shader_programs::{
    BuiltinTemplate, TemplateManifest, Variant, builtin_source, compile_template, effect_programs,
};
use rikka_slang::{Compiler, Optimization};
use rikka_spirv::Facts;

type Failure = Box<dyn std::error::Error>;

const USAGE: &str = "usage:
  programs template --id ID --output DIRECTORY [--module NAME=PATH]...
  programs effect --output DIRECTORY [--module NAME=PATH]... SOURCE
  programs listing --output FILE [--template ID=INCLUDED=DIRECTORY]... [--effect INCLUDED=DIRECTORY]...";

const MANIFEST: &str = "manifest.json";

const SPIRV_EXTENSION: &str = "spv";

const LAYOUT_EXTENSION: &str = "json";

#[derive(Default)]
struct Arguments {
    id: Option<String>,
    output: Option<PathBuf>,
    modules: Vec<(String, PathBuf)>,
    templates: Vec<(String, String, PathBuf)>,
    effects: Vec<(String, PathBuf)>,
    source: Option<PathBuf>,
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
            "--id" => arguments.id = Some(value()?),
            "--output" => arguments.output = Some(value()?.into()),
            "--module" => {
                let (name, path) = pair(&value()?, "NAME=PATH")?;
                arguments.modules.push((name, path.into()));
            }
            "--template" => {
                let (id, location) = pair(&value()?, "ID=INCLUDED=DIRECTORY")?;
                let (included, directory) = pair(&location, "ID=INCLUDED=DIRECTORY")?;
                arguments.templates.push((id, included, directory.into()));
            }
            "--effect" => {
                let (included, directory) = pair(&value()?, "INCLUDED=DIRECTORY")?;
                arguments.effects.push((included, directory.into()));
            }
            _ => arguments.source = Some(argument.into()),
        }
    }
    Ok(arguments)
}

fn required<T>(value: Option<T>, flag: &str) -> Result<T, Failure> {
    value.ok_or_else(|| format!("{flag} is required\n{USAGE}").into())
}

fn read(path: &Path) -> Result<String, Failure> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()).into())
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
        builder = builder.module(name, read(path)?);
    }
    Ok(builder.build()?)
}

fn template(arguments: Arguments) -> Result<(), Failure> {
    let id = required(arguments.id, "--id")?;
    let output = required(arguments.output, "--output")?;
    let builtin = BuiltinTemplate::from_id(&id).ok_or_else(|| format!("no template {id}"))?;
    let compiled = compile_template(
        &compiler(&arguments.modules, Optimization::High)?,
        Some(&compiler(&arguments.modules, Optimization::None)?),
        builtin.id(),
        &builtin_source(builtin.module()),
    )
    .map_err(|error| format!("{id}: {error}"))?;
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

fn effect(arguments: Arguments) -> Result<(), Failure> {
    let source = required(arguments.source, "SOURCE")?;
    let output = required(arguments.output, "--output")?;
    let name = source
        .file_stem()
        .ok_or("SOURCE names no file")?
        .to_string_lossy()
        .into_owned();
    let programs = effect_programs(
        &compiler(&arguments.modules, Optimization::High)?,
        &name,
        &read(&source)?,
    )
    .map_err(|error| format!("{}: {error}", source.display()))?;
    if programs.is_empty() {
        return Err(format!("{} declares no entry point", source.display()).into());
    }
    std::fs::create_dir_all(&output)?;
    for program in programs {
        write(
            &output.join(format!("{}.{SPIRV_EXTENSION}", program.entry)),
            rikka_spirv::bytes(&program.spirv),
        )?;
        write(
            &output.join(format!("{}.{LAYOUT_EXTENSION}", program.entry)),
            serde_json::to_string(&program.layout)?,
        )?;
    }
    Ok(())
}

fn entries(directory: &Path) -> Result<Vec<String>, Failure> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|kind| kind == SPIRV_EXTENSION)
            && let Some(stem) = path.file_stem()
        {
            found.push(stem.to_string_lossy().into_owned());
        }
    }
    found.sort();
    Ok(found)
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
        "pub static TEMPLATES: [rikka_shader_programs::TemplateArtifact; rikka_shader_programs::BuiltinTemplate::COUNT] = [\n",
    );
    for builtin in BuiltinTemplate::ALL {
        let (_, included, directory) = arguments
            .templates
            .iter()
            .find(|(id, _, _)| id == builtin.id())
            .ok_or_else(|| format!("template {} is not built", builtin.id()))?;
        let variants = Variant::ALL
            .iter()
            .map(|variant| shader(directory, included, variant.entry()))
            .collect::<Result<Vec<_>, Failure>>()?
            .join(", ");
        writeln!(
            text,
            "    rikka_shader_programs::TemplateArtifact {{ manifest: include_str!(\"{included}/{MANIFEST}\"), variants: [{variants}] }},"
        )?;
    }
    if let Some((id, _, _)) = arguments
        .templates
        .iter()
        .find(|(id, _, _)| BuiltinTemplate::from_id(id).is_none())
    {
        return Err(format!("{id} is not a built-in template").into());
    }
    text.push_str("];\n\npub static EFFECTS: &[rikka_shader_programs::EffectArtifact] = &[\n");
    let mut effects = arguments.effects;
    effects.sort();
    for (included, directory) in &effects {
        let program = Path::new(included)
            .file_name()
            .ok_or("--effect names no folder")?
            .to_string_lossy();
        for entry in entries(directory)? {
            let spirv = shader(directory, included, &entry)?;
            writeln!(
                text,
                "    rikka_shader_programs::EffectArtifact {{ program: {program:?}, entry: {entry:?}, spirv: {spirv}, layout: include_str!(\"{included}/{entry}.{LAYOUT_EXTENSION}\") }},"
            )?;
        }
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
        Some("effect") => effect(arguments),
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
