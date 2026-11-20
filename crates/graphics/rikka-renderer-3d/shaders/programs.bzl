"""Builds the renderer's built-in material templates and effect programs into `src/programs.rs`."""

load("//bazel/slang:defs.bzl", "SlangModulesInfo")

_VALIDATION = {"SLANG_RUN_SPIRV_VALIDATION": "1"}

def _stem(file):
    return file.basename[:-len(file.extension) - 1]

def _shader_programs_impl(ctx):
    library = ctx.attr.library[SlangModulesInfo]
    modules = ctx.actions.args()
    for module in library.modules:
        modules.add("--module", "{}={}".format(module.name, module.file.path))

    listed = ctx.actions.args()
    built = []
    for template in ctx.attr.templates:
        included = "programs/templates/" + template.replace("/", "_")
        directory = ctx.actions.declare_directory("src/" + included)
        args = ctx.actions.args()
        args.add("--id", template)
        args.add("--output", directory.path)
        ctx.actions.run(
            executable = ctx.executable._tool,
            arguments = ["template", args, modules],
            inputs = library.files,
            outputs = [directory],
            env = _VALIDATION,
            mnemonic = "MaterialTemplate",
            progress_message = "Compiling the material template {}".format(template),
        )
        listed.add("--template", "{}={}={}".format(template, included, directory.path))
        built.append(directory)

    for source in ctx.files.effects:
        included = "programs/effects/" + _stem(source)
        directory = ctx.actions.declare_directory("src/" + included)
        args = ctx.actions.args()
        args.add("--output", directory.path)
        args.add(source)
        ctx.actions.run(
            executable = ctx.executable._tool,
            arguments = ["effect", args, modules],
            inputs = depset([source], transitive = [library.files]),
            outputs = [directory],
            env = _VALIDATION,
            mnemonic = "EffectProgram",
            progress_message = "Compiling the effect program %{input}",
        )
        listed.add("--effect", "{}={}".format(included, directory.path))
        built.append(directory)

    listing = ctx.actions.declare_file("src/programs.rs")
    listed.add("--output", listing)
    ctx.actions.run(
        executable = ctx.executable._tool,
        arguments = ["listing", listed],
        inputs = built,
        outputs = [listing],
        mnemonic = "ShaderPrograms",
        progress_message = "Listing the shader programs of %{label}",
    )
    return [
        DefaultInfo(files = depset([listing])),
        OutputGroupInfo(compile_data = depset(built)),
    ]

shader_programs = rule(
    implementation = _shader_programs_impl,
    doc = "Compiles built-in material templates and effect programs, one action each.",
    attrs = {
        "effects": attr.label_list(
            doc = "Effect programs; every entry point becomes an `EffectArtifact`.",
            allow_files = [".slang"],
        ),
        "library": attr.label(mandatory = True, providers = [SlangModulesInfo]),
        "templates": attr.string_list(doc = "Ids of the built-in material templates."),
        "_tool": attr.label(
            cfg = "exec",
            default = Label("//crates/graphics/rikka-renderer-3d/shaders:programs-tool"),
            executable = True,
        ),
    },
)
