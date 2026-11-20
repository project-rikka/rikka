use std::path::{Path, PathBuf};

use rikka_shader_build::{Bundled, Compiler, Error, Module, Objects, declarations, unused_modules};

const COMMON: &str = "float brighten(float value) { return value * 2.0; }\n";

const UMBRELLA: &str = "__exported import demo.common;\n";

const PROGRAM: &str = r#"import demo;

[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] RWTexture2D<float> target[];

[shader("compute")]
[numthreads(8, 8, 1)]
void fill(uniform uint offset, uint3 id : SV_DispatchThreadID) {
    target[0][id.xy] = brighten(float(id.x + offset));
}

[shader("fragment")]
float4 shade(float4 position : SV_Position) : SV_Target {
    return float4(brighten(position.x));
}
"#;

const COHERENT: &str = r#"struct Push { uint* flags; }

[shader("compute")]
[numthreads(64, 1, 1)]
void publish(uniform Push push, uint3 id : SV_DispatchThreadID) {
    storeCoherent<4, MemoryScope.Workgroup>(push.flags + id.x, 1u);
}
"#;

const VULKAN_MEMORY_MODEL: [u32; 2] = [0x0002_0011, 5345];

struct Package {
    root: PathBuf,
}

impl Package {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("rikka-shader-build-{name}"));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("clear scratch package");
        }
        std::fs::create_dir_all(&root).expect("create scratch package");
        Self { root }
    }

    fn file(&self, relative: &str, text: &str) -> &Self {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("file has a parent")).expect("create dirs");
        std::fs::write(path, text).expect("write source");
        self
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn compiler(&self) -> Compiler {
        Compiler::new(env!("SLANGC")).include(self.path("lib"))
    }

    fn module(&self, name: &str, relative: &str) -> Module {
        Module {
            path: self.path(relative),
            bundled: bundled(name, relative),
        }
    }
}

fn bundled(name: &str, relative: &str) -> Bundled {
    Bundled {
        name: name.to_owned(),
        included: format!("../{relative}"),
    }
}

fn words(path: &Path) -> Vec<u32> {
    std::fs::read(path)
        .expect("compiled object exists")
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect()
}

#[test]
fn entry_points_become_constants_and_library_is_bundled() {
    let package = Package::new("entries");
    package
        .file("lib/demo.slang", UMBRELLA)
        .file("lib/demo/common.slang", COMMON)
        .file("three/paint.slang", PROGRAM);
    let objects = package.path("out/paint");
    let compiled = package
        .compiler()
        .object(&package.path("three/paint.slang"), &objects)
        .expect("compiles");
    assert_eq!(compiled.objects.len(), 2);
    for object in &compiled.objects {
        assert_eq!(
            words(object)[0],
            0x0723_0203,
            "{} is SPIR-V",
            object.display()
        );
    }

    let shaders = declarations(
        &[Objects {
            directory: objects,
            included: "spv/paint".to_owned(),
        }],
        &[
            package.module("demo", "lib/demo.slang"),
            package.module("demo.common", "lib/demo/common.slang"),
        ],
    )
    .expect("declares");
    for expected in [
        "(\"demo\", include_str!(\"../lib/demo.slang\"))",
        "(\"demo.common\", include_str!(\"../lib/demo/common.slang\"))",
    ] {
        assert!(
            shaders.contains(expected),
            "{expected} missing from {shaders}"
        );
    }
    for (name, stage, storage, push) in [
        ("PAINT_COMP", "comp", true, 4),
        ("PAINT_FRAG", "frag", false, 0),
    ] {
        let start = format!(
            "pub static {name}: rikka_spirv::Shader = rikka_spirv::Shader {{ spirv: \
             include_bytes!(\"spv/paint/paint.{stage}.spv\"), hash: 0x"
        );
        let line = shaders
            .lines()
            .find(|line| line.starts_with(&start))
            .unwrap_or_else(|| panic!("{start} missing from {shaders}"));
        assert!(
            line.ends_with(&format!(
                "uses_storage_images: {storage}, push_bytes: {push} }};"
            )),
            "{line}"
        );
    }
}

#[test]
fn vertex_position_is_invariant() {
    let package = Package::new("invariant");
    package.file(
        "mesh.slang",
        "[shader(\"vertex\")] float4 place(uint vertex : SV_VulkanVertexID) : SV_Position \
         { return float4(float(vertex), 0.0, 0.0, 1.0); }\n",
    );
    let compiled = package
        .compiler()
        .object(&package.path("mesh.slang"), &package.path("out"))
        .expect("compiles");
    let words = words(&compiled.objects[0]);
    let decorations: Vec<&[u32]> = words
        .windows(4)
        .filter(|window| window[0] == (4 << 16) | 71 && window[2] == 11 && window[3] == 0)
        .collect();
    let position = decorations.first().expect("a Position built-in")[1];
    assert!(
        words
            .windows(3)
            .any(|window| window == [(3 << 16) | 71, position, 18]),
        "Position is decorated Invariant"
    );
}

type Expected = fn(&Error) -> bool;

#[test]
fn malformed_slang_sources_are_refused() {
    let cases: [(&str, &str, Expected); 3] = [
        ("missing-entry", COMMON, |error| {
            matches!(error, Error::MissingEntryPoint(_))
        }),
        (
            "duplicate-stage",
            "[shader(\"fragment\")] float4 a() : SV_Target { return 0; }\n\
             [shader(\"fragment\")] float4 b() : SV_Target { return 1; }\n",
            |error| matches!(error, Error::DuplicateStage { .. }),
        ),
        (
            "compile-error",
            "[shader(\"fragment\")] float4 a() : SV_Target { return missing; }\n",
            |error| matches!(error, Error::Compile { report, .. } if report.contains("missing")),
        ),
    ];
    for (name, text, expected) in cases {
        let package = Package::new(name);
        package.file("paint.slang", text);
        let error = package
            .compiler()
            .object(&package.path("paint.slang"), &package.path("out"))
            .expect_err(name);
        assert!(expected(&error), "{name}: {error}");
    }

    let package = Package::new("library-entry");
    package.file("lib/demo.slang", PROGRAM);
    let error = declarations(&[], &[package.module("demo", "lib/demo.slang")])
        .expect_err("a library module with an entry point");
    assert!(matches!(error, Error::LibraryEntryPoint(_)), "{error}");
}

#[test]
fn vulkan_memory_model_admits_coherent_accesses() {
    let package = Package::new("memory-model");
    package.file("publish.slang", COHERENT);
    let source = package.path("publish.slang");
    let output = package.path("out");
    let error = package
        .compiler()
        .object(&source, &output)
        .expect_err("coherent access without the model");
    assert!(matches!(error, Error::Compile { .. }), "{error}");

    let compiled = package
        .compiler()
        .vulkan_memory_model()
        .object(&source, &output)
        .expect("compiles under the Vulkan memory model");
    assert!(
        words(&compiled.objects[0])
            .windows(2)
            .any(|pair| pair == VULKAN_MEMORY_MODEL),
        "the module declares the VulkanMemoryModel capability"
    );
}

#[test]
fn modules_a_shader_never_imports_are_reported_unused() {
    let package = Package::new("unused");
    package
        .file("lib/demo.slang", UMBRELLA)
        .file("lib/demo/common.slang", COMMON)
        .file("lib/other/idle.slang", COMMON)
        .file("paint.slang", PROGRAM);
    let compiler = package.compiler();
    let compiled = compiler
        .object(&package.path("paint.slang"), &package.path("out"))
        .expect("compiles");
    assert_eq!(
        unused_modules(compiler.includes(), &compiled.sources).expect("lists"),
        [package.path("lib/other/idle.slang")]
    );
    assert!(
        std::fs::read_dir(package.path("out"))
            .expect("objects exist")
            .all(|entry| entry
                .expect("an entry")
                .path()
                .extension()
                .is_some_and(|kind| kind == "spv")),
        "only objects are left in the output"
    );
}
