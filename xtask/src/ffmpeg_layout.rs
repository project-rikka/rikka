use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::host;

const MANIFESTS: &str = "manifests";

struct Package {
    root: &'static str,
    generated: &'static str,
    modules: &'static str,
}

const PACKAGES: &[Package] = &[
    Package {
        root: "crates/media/rikka-ffmpeg",
        generated: "src/layout_asserts.rs",
        modules: "crate",
    },
    Package {
        root: "crates/media/rikka-video-core",
        generated: "src/ffi/layout_asserts.rs",
        modules: "crate::ffi",
    },
];

const LIBRARIES: &[&str] = &["libavutil", "libavcodec", "libavformat", "libswscale"];

const HEADERS: &[&str] = &[
    "libavutil/avutil.h",
    "libavutil/frame.h",
    "libavutil/buffer.h",
    "libavutil/dict.h",
    "libavutil/error.h",
    "libavutil/log.h",
    "libavutil/mathematics.h",
    "libavutil/opt.h",
    "libavutil/pixdesc.h",
    "libavutil/hwcontext.h",
    "libavutil/hwcontext_vulkan.h",
    "libavcodec/avcodec.h",
    "libavformat/avformat.h",
    "libswscale/swscale.h",
];

const C_EXPRESSIONS: &[(&str, &str)] = &[
    ("AVERROR_EAGAIN", "AVERROR(EAGAIN)"),
    ("AVERROR_ENOMEM", "AVERROR(ENOMEM)"),
    ("AVERROR_EINVAL", "AVERROR(EINVAL)"),
    ("AVERROR_EIO", "AVERROR(EIO)"),
];

const RUST_KEYWORDS: &[&str] = &[
    "type", "loop", "ref", "box", "priv", "in", "fn", "impl", "match",
];

enum Entry {
    Size {
        module: String,
        name: String,
    },
    Offset {
        module: String,
        name: String,
        field: String,
    },
    Constant {
        module: String,
        name: String,
    },
}

pub(crate) fn run(workspace: &Path) -> Result<()> {
    let cflags = ffmpeg_cflags()?;
    let vulkan = vulkan_include()?;
    let compiler = host::which("clang")
        .or_else(|| host::which("cc"))
        .context("`clang` or `cc` is needed to compile the layout probe")?;
    let versions = versions()?;
    for package in PACKAGES {
        let root = workspace.join(package.root);
        let entries = read_manifests(&root.join(MANIFESTS))?;
        let staging = staging()?;
        let printed = probe(&staging, &compiler, &cflags, &vulkan, package, &entries);
        host::remove(&staging)?;

        let generated = root.join(package.generated);
        host::write(&generated, &(header() + &printed?))?;
        host::rustfmt(&generated)?;

        println!("checked {} layouts against {versions}", entries.len());
        println!("wrote {}", generated.display());
    }
    Ok(())
}

fn header() -> String {
    "#![allow(clippy::unnecessary_cast)]\n\n".to_owned()
}

fn probe(
    staging: &Path,
    compiler: &Path,
    cflags: &[String],
    vulkan: &Path,
    package: &Package,
    entries: &[Entry],
) -> Result<String> {
    let source = staging.join("probe.c");
    let program = staging.join("probe");
    host::write(&source, probe_source(package.modules, entries))?;
    host::run(
        Command::new(compiler)
            .args(cflags)
            .arg(format!("-I{}", vulkan.display()))
            .arg(&source)
            .arg("-o")
            .arg(&program),
    )
    .context(
        "layout probe failed to compile; a transcribed field is missing from the installed headers",
    )?;
    host::output(&mut Command::new(&program))
}

fn probe_source(modules: &str, entries: &[Entry]) -> String {
    let includes: String = HEADERS
        .iter()
        .map(|header| format!("#include <{header}>\n"))
        .collect();
    let checks: String = entries
        .iter()
        .map(|entry| format!("    {}\n", printf_for(modules, entry)))
        .collect();
    format!(
        "#include <stdio.h>\n#include <stddef.h>\n{includes}int main(void) {{\n{checks}    return 0;\n}}\n"
    )
}

fn printf_for(modules: &str, entry: &Entry) -> String {
    match entry {
        Entry::Size { module, name } => format!(
            "printf(\"const _: () = assert!(size_of::<{modules}::{module}::{name}>() == \
             %zu);\\n\", sizeof({name}));"
        ),
        Entry::Offset {
            module,
            name,
            field,
        } => {
            let spelled = rust_field(field);
            format!(
                "printf(\"const _: () = assert!(std::mem::offset_of!({modules}::{module}::\
                 {name}, {spelled}) == %zu);\\n\", offsetof({name}, {field}));"
            )
        }
        Entry::Constant { module, name } => {
            let expression = C_EXPRESSIONS
                .iter()
                .find(|(rust, _)| rust == name)
                .map_or(name.as_str(), |(_, c)| c);
            format!(
                "printf(\"const _: () = assert!({modules}::{module}::{name} as i64 == \
                 %lld);\\n\", (long long)({expression}));"
            )
        }
    }
}

fn rust_field(field: &str) -> String {
    if RUST_KEYWORDS.contains(&field) {
        format!("r#{field}")
    } else {
        field.to_owned()
    }
}

fn read_manifests(directory: &Path) -> Result<Vec<Entry>> {
    let mut manifests: Vec<PathBuf> = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to read {}", directory.display()))?
        .into_iter()
        .map(|manifest| manifest.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "manifest"))
        .collect();
    manifests.sort();

    let mut entries = Vec::new();
    for manifest in &manifests {
        let name = manifest.file_name().unwrap_or_default().to_string_lossy();
        for line in host::read(manifest)?.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["SIZEOF", module, structure] => entries.push(Entry::Size {
                    module: (*module).to_owned(),
                    name: (*structure).to_owned(),
                }),
                ["OFFSET", module, structure, field] => entries.push(Entry::Offset {
                    module: (*module).to_owned(),
                    name: (*structure).to_owned(),
                    field: (*field).to_owned(),
                }),
                ["CONST", module, constant, _type] => entries.push(Entry::Constant {
                    module: (*module).to_owned(),
                    name: (*constant).to_owned(),
                }),
                ["PREFIX", ..] => {}
                _ => bail!("{name}: unknown manifest entry: {line}"),
            }
        }
    }
    Ok(entries)
}

fn ffmpeg_cflags() -> Result<Vec<String>> {
    let pkg_config = pkg_config()?;
    for library in LIBRARIES {
        let found = Command::new(&pkg_config)
            .args(["--exists", library])
            .status()
            .with_context(|| format!("asking pkg-config about {library}"))?;
        if !found.success() {
            bail!(
                "{library}'s headers are not installed -- this task needs the FFmpeg \
                 development package (Arch: `ffmpeg`; Debian: `{library}-dev`). Only \
                 this task needs them: rikka-ffmpeg opens FFmpeg by soname at runtime."
            );
        }
    }
    let cflags = host::output(Command::new(&pkg_config).arg("--cflags").args(LIBRARIES))?;
    Ok(cflags.split_whitespace().map(str::to_owned).collect())
}

fn vulkan_include() -> Result<PathBuf> {
    let sdk = std::env::var_os("VULKAN_SDK").map(|sdk| PathBuf::from(sdk).join("include"));
    let candidates = sdk.into_iter().chain([PathBuf::from("/usr/include")]);
    for candidate in candidates {
        if candidate.join("vulkan").join("vulkan.h").is_file() {
            return Ok(candidate);
        }
    }
    bail!(
        "`vulkan/vulkan.h` was not found, and libavutil's hwcontext_vulkan.h includes \
         it -- install the Vulkan headers (Arch: `vulkan-headers`; Debian: \
         `libvulkan-dev`), or point VULKAN_SDK at an SDK"
    )
}

fn versions() -> Result<String> {
    let pkg_config = pkg_config()?;
    let stdout = host::output(Command::new(pkg_config).arg("--modversion").args(LIBRARIES))?;
    Ok(LIBRARIES
        .iter()
        .zip(stdout.lines())
        .map(|(library, version)| format!("{library} {version}"))
        .collect::<Vec<_>>()
        .join(", "))
}

fn pkg_config() -> Result<PathBuf> {
    host::which("pkg-config")
        .or_else(|| host::which("pkgconf"))
        .context("`pkg-config` is needed to find the FFmpeg headers")
}

fn staging() -> Result<PathBuf> {
    let staging = std::env::temp_dir().join(format!("rikka-ffmpeg-layout-{}", std::process::id()));
    host::reset_dir(&staging)?;
    Ok(staging)
}
