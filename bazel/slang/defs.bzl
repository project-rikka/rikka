"""Slang shader packages: one action per shader, one Rust crate of `rikka_spirv::Shader` statics."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("@nix//:paths.bzl", "NIX")
load("//bazel/rust:defs.bzl", "rikka_library")

SlangModulesInfo = provider(
    doc = "Importable Slang modules under one include directory.",
    fields = {
        "directory": "include directory, relative to the execution root",
        "files": "depset of the modules' sources",
        "modules": "list of struct(name, file), `name` as written in `import`",
    },
)

_SLANG = ".slang"

def _stem(file):
    return file.basename[:-len(_SLANG)]

def _package_relative(ctx, file):
    return paths.relativize(file.short_path, ctx.label.package)

def _slang_modules_impl(ctx):
    root = paths.join(ctx.label.package, ctx.attr.root)
    modules = [
        struct(
            name = paths.replace_extension(paths.relativize(source.short_path, root), "").replace("/", "."),
            file = source,
        )
        for source in ctx.files.srcs
    ]
    files = depset(ctx.files.srcs)
    return [
        DefaultInfo(files = files),
        SlangModulesInfo(
            directory = paths.join(ctx.label.workspace_root, root),
            files = files,
            modules = modules,
        ),
    ]

slang_modules = rule(
    implementation = _slang_modules_impl,
    doc = "Slang modules importable as `import a.b;` from `<root>/a/b.slang`.",
    attrs = {
        "root": attr.string(default = "lib"),
        "srcs": attr.label_list(allow_files = [_SLANG]),
    },
)

def _compiler_args(ctx, modules):
    args = ctx.actions.args()
    args.add("--slangc", ctx.attr.slangc)
    for info in modules:
        args.add("-I", info.directory)
    if ctx.attr.vulkan_memory_model:
        args.add("--vulkan-memory-model")
    if ctx.attr.precise_floating_point:
        args.add("--precise-floating-point")
    return args

def _compile(ctx, compiler, imported, source, output, unused):
    args = ctx.actions.args()
    args.add("--output", output.path)
    args.add("--unused-inputs", unused)
    args.add(source)
    ctx.actions.run(
        executable = ctx.executable._compiler,
        arguments = ["object", compiler, args],
        inputs = depset([source], transitive = [imported]),
        outputs = [output, unused],
        unused_inputs_list = unused,
        mnemonic = "SlangObject",
        progress_message = "Compiling Slang shader %{input}",
    )

def _slang_shaders_impl(ctx):
    library = ctx.attr.library[SlangModulesInfo]
    modules = [library] + [target[SlangModulesInfo] for target in ctx.attr.imports]
    imported = depset(transitive = [info.files for info in modules])
    compiler = _compiler_args(ctx, modules)

    objects = []
    stems = {}
    for source in ctx.files.srcs:
        stem = _stem(source)
        if stem in stems:
            fail("{} and {} share the stem {}".format(stems[stem].short_path, source.short_path, stem))
        stems[stem] = source
        output = ctx.actions.declare_directory("src/spv/" + stem)
        unused = ctx.actions.declare_file("src/spv/{}.unused".format(stem))
        _compile(ctx, compiler, imported, source, output, unused)
        objects.append((stem, output))

    declarations = ctx.actions.declare_file("src/shaders.rs")
    args = ctx.actions.args()
    args.add("--output", declarations)
    for stem, output in objects:
        args.add("--objects", "{}=spv/{}".format(output.path, stem))
    for module in library.modules:
        args.add("--module", "{}={}=../{}".format(module.name, module.file.path, _package_relative(ctx, module.file)))
    ctx.actions.run(
        executable = ctx.executable._compiler,
        arguments = ["declarations", args],
        inputs = depset([output for _, output in objects], transitive = [library.files]),
        outputs = [declarations],
        mnemonic = "SlangDeclarations",
        progress_message = "Declaring the shaders of %{label}",
    )

    return [
        DefaultInfo(files = depset([declarations])),
        OutputGroupInfo(compile_data = depset([output for _, output in objects])),
    ]

slang_shaders = rule(
    implementation = _slang_shaders_impl,
    doc = "Compiles Slang shaders to SPIR-V and declares them in `src/shaders.rs`.",
    attrs = {
        "imports": attr.label_list(
            doc = "Slang modules of other packages on the include path.",
            providers = [SlangModulesInfo],
        ),
        "library": attr.label(
            doc = "This package's own modules, bundled as `SLANG_LIBRARY`.",
            mandatory = True,
            providers = [SlangModulesInfo],
        ),
        "precise_floating_point": attr.bool(),
        "slangc": attr.string(mandatory = True),
        "srcs": attr.label_list(
            doc = "Shaders with at most one entry point per stage; each becomes `<STEM>_<STAGE>` statics.",
            allow_files = [_SLANG],
        ),
        "vulkan_memory_model": attr.bool(),
        "_compiler": attr.label(
            cfg = "exec",
            default = Label("//crates/graphics/rikka-shader-build:compile"),
            executable = True,
        ),
    },
)

def rikka_shaders(
        name,
        exclude = [],
        imports = [],
        vulkan_memory_model = False,
        precise_floating_point = False,
        **kwargs):
    """A crate of `rikka_spirv::Shader` statics compiled from this package's Slang shaders.

    `lib/**/*.slang` are importable modules and every other `.slang` file declares
    entry points. `src/lib.rs` includes `shaders.rs`.

    Args:
        name: crate target name.
        exclude: globs of `.slang` files other rules of the package compile.
        imports: `slang_modules` of other packages.
        vulkan_memory_model: compile under the Vulkan memory model.
        precise_floating_point: forbid floating-point contraction.
        **kwargs: passed to `rikka_library`.
    """
    library = native.glob(["lib/**/*.slang"], allow_empty = True)
    slang_modules(
        name = "lib",
        srcs = library,
        visibility = ["//crates:__subpackages__"],
    )
    slang_shaders(
        name = name + "_spirv",
        srcs = native.glob(["**/*.slang"], exclude = ["lib/**"] + exclude),
        imports = imports,
        library = ":lib",
        precise_floating_point = precise_floating_point,
        slangc = NIX["slangc"],
        vulkan_memory_model = vulkan_memory_model,
    )
    native.filegroup(
        name = name + "_objects",
        srcs = [":" + name + "_spirv"],
        output_group = "compile_data",
    )
    rikka_library(
        name = name,
        srcs = native.glob(["src/**/*.rs"]) + kwargs.pop("srcs", []) + [":" + name + "_spirv"],
        compile_data = kwargs.pop("compile_data", []) + [":" + name + "_objects"] + library,
        deps = kwargs.pop("deps", []) + ["//crates/graphics/rikka-spirv"],
        **kwargs
    )
