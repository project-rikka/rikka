use std::path::PathBuf;
use std::process::ExitCode;

use rikka_shader_build::{
    Bundled, Compiler, Error, Module, Objects, Result, unused_modules, write_declarations,
};

const USAGE: &str = "usage:
  compile object --slangc PATH [-I DIRECTORY]... [--vulkan-memory-model] [--precise-floating-point]
                 --output DIRECTORY [--unused-inputs FILE] SOURCE
  compile declarations --output FILE [--objects DIRECTORY=INCLUDED]...
                       [--module NAME=PATH=INCLUDED]...";

struct Arguments {
    rest: std::vec::IntoIter<String>,
}

impl Arguments {
    fn value(&mut self, flag: &str) -> Result<String> {
        self.rest
            .next()
            .ok_or_else(|| Error::Usage(format!("{flag} needs a value\n{USAGE}")))
    }
}

fn required<T>(value: Option<T>, flag: &str) -> Result<T> {
    value.ok_or_else(|| Error::Usage(format!("{flag} is required\n{USAGE}")))
}

fn pair<'a>(argument: &'a str, shape: &str) -> Result<(&'a str, &'a str)> {
    argument
        .split_once('=')
        .ok_or_else(|| Error::Usage(format!("expected {shape}, got {argument}")))
}

fn object(mut arguments: Arguments) -> Result<()> {
    let (mut slangc, mut output, mut unused, mut source) = (None, None, None, None);
    let mut includes = Vec::new();
    let (mut vulkan_memory_model, mut precise_floating_point) = (false, false);
    while let Some(argument) = arguments.rest.next() {
        match argument.as_str() {
            "--slangc" => slangc = Some(PathBuf::from(arguments.value("--slangc")?)),
            "-I" => includes.push(PathBuf::from(arguments.value("-I")?)),
            "--vulkan-memory-model" => vulkan_memory_model = true,
            "--precise-floating-point" => precise_floating_point = true,
            "--output" => output = Some(PathBuf::from(arguments.value("--output")?)),
            "--unused-inputs" => unused = Some(PathBuf::from(arguments.value("--unused-inputs")?)),
            _ => source = Some(PathBuf::from(argument)),
        }
    }
    let mut compiler = includes.into_iter().fold(
        Compiler::new(required(slangc, "--slangc")?),
        Compiler::include,
    );
    if vulkan_memory_model {
        compiler = compiler.vulkan_memory_model();
    }
    if precise_floating_point {
        compiler = compiler.precise_floating_point();
    }
    let compiled = compiler.object(&required(source, "SOURCE")?, &required(output, "--output")?)?;
    let Some(unused) = unused else {
        return Ok(());
    };
    let listed: String = unused_modules(compiler.includes(), &compiled.sources)?
        .iter()
        .map(|module| format!("{}\n", module.display()))
        .collect();
    std::fs::write(&unused, listed).map_err(|source| Error::Write {
        path: unused,
        source,
    })
}

fn declarations(mut arguments: Arguments) -> Result<()> {
    let mut output = None;
    let (mut objects, mut library) = (Vec::new(), Vec::new());
    while let Some(argument) = arguments.rest.next() {
        match argument.as_str() {
            "--output" => output = Some(PathBuf::from(arguments.value("--output")?)),
            "--objects" => {
                let value = arguments.value("--objects")?;
                let (directory, included) = pair(&value, "DIRECTORY=INCLUDED")?;
                objects.push(Objects {
                    directory: directory.into(),
                    included: included.to_owned(),
                });
            }
            "--module" => {
                let value = arguments.value("--module")?;
                let (name, rest) = pair(&value, "NAME=PATH=INCLUDED")?;
                let (path, included) = pair(rest, "NAME=PATH=INCLUDED")?;
                library.push(Module {
                    path: path.into(),
                    bundled: Bundled {
                        name: name.to_owned(),
                        included: included.to_owned(),
                    },
                });
            }
            unknown => return Err(Error::Usage(format!("unknown argument {unknown}\n{USAGE}"))),
        }
    }
    write_declarations(&required(output, "--output")?, &objects, &library)
}

fn run() -> Result<()> {
    let mut arguments = Arguments {
        rest: std::env::args().skip(1).collect::<Vec<_>>().into_iter(),
    };
    match arguments.rest.next().as_deref() {
        Some("object") => object(arguments),
        Some("declarations") => declarations(arguments),
        _ => Err(Error::Usage(USAGE.to_owned())),
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
