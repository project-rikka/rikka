"""Builds every listed shader preset graph into a material template artifact in `src/presets.rs`."""

load("//bazel/slang:defs.bzl", "SlangModulesInfo")

_VALIDATION = {"SLANG_RUN_SPIRV_VALIDATION": "1"}

def _preset_programs_impl(ctx):
    library = ctx.attr.library[SlangModulesInfo]
    modules = ctx.actions.args()
    for module in library.modules:
        modules.add("--module", "{}={}".format(module.name, module.file.path))

    listed = ctx.actions.args()
    built = []
    for key in ctx.attr.presets:
        included = "presets/" + key
        directory = ctx.actions.declare_directory("src/" + included)
        args = ctx.actions.args()
        args.add("--key", key)
        args.add("--output", directory.path)
        ctx.actions.run(
            executable = ctx.executable._tool,
            arguments = ["template", args, modules],
            inputs = library.files,
            outputs = [directory],
            env = _VALIDATION,
            mnemonic = "PresetTemplate",
            progress_message = "Compiling the shader preset {}".format(key),
        )
        listed.add("--preset", "{}={}={}".format(key, included, directory.path))
        built.append(directory)

    listing = ctx.actions.declare_file("src/presets.rs")
    listed.add("--output", listing)
    ctx.actions.run(
        executable = ctx.executable._tool,
        arguments = ["listing", listed],
        inputs = built,
        outputs = [listing],
        mnemonic = "PresetListing",
        progress_message = "Listing the shader presets of %{label}",
    )
    return [
        DefaultInfo(files = depset([listing])),
        OutputGroupInfo(compile_data = depset(built)),
    ]

preset_programs = rule(
    implementation = _preset_programs_impl,
    doc = "Compiles shader preset graphs into material templates, one action each.",
    attrs = {
        "library": attr.label(mandatory = True, providers = [SlangModulesInfo]),
        "presets": attr.string_list(doc = "Keys of the presets to build."),
        "_tool": attr.label(
            cfg = "exec",
            default = Label("//crates/graphics/rikka-shader-presets-gpu:presets-tool"),
            executable = True,
        ),
    },
)
