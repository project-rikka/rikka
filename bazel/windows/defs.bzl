"""Windows executables and the portable Windows distribution."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@nix//:paths.bzl", "NIX")
load("//bazel/rust:defs.bzl", "rikka_binary")

_WINDOWS = "@platforms//os:windows"

_PRODUCT = "Rikka"

def _version_numbers(version):
    numbers = version.split(".")
    if len(numbers) != 3 or not all([number.isdigit() and int(number) <= 65535 for number in numbers]):
        fail("version {} is not major.minor.patch with parts up to 65535".format(version))
    return numbers + ["0"]

def _rc_string(text):
    return text.replace("\"", "\"\"")

def _xml_text(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def _windows_resource_impl(ctx):
    version = ctx.attr._version[BuildSettingInfo].value
    numbers = _version_numbers(version)
    description = "{} {}".format(_PRODUCT, ctx.attr.product)
    identity = "{}.{}".format(_PRODUCT, ctx.attr.product.replace(" ", ""))

    manifest = ctx.actions.declare_file(ctx.attr.executable + ".manifest")
    ctx.actions.expand_template(
        template = ctx.file._manifest,
        output = manifest,
        substitutions = {
            "@DESCRIPTION@": _xml_text(description),
            "@IDENTITY@": identity,
            "@VERSION@": ".".join(numbers),
        },
    )

    script = ctx.actions.declare_file(ctx.attr.executable + ".rc")
    ctx.actions.expand_template(
        template = ctx.file._script,
        output = script,
        substitutions = {
            "@DESCRIPTION@": _rc_string(description),
            "@EXECUTABLE@": _rc_string(ctx.attr.executable),
            "@MANIFEST@": manifest.basename,
            "@VERSION@": version,
            "@VERSION_COMMAS@": ",".join(numbers),
        },
    )

    resource = ctx.actions.declare_file(ctx.attr.executable + ".res.o")
    ctx.actions.run(
        executable = NIX["windows"]["windres"],
        arguments = [
            "--input-format=rc",
            "--output-format=coff",
            "--include-dir=" + paths.dirname(script.path),
            "--input=" + script.path,
            "--output=" + resource.path,
        ],
        inputs = [script, manifest],
        outputs = [resource],
        mnemonic = "WindowsResource",
        progress_message = "Compiling the Windows resources of %{label}",
    )
    return [DefaultInfo(files = depset([resource]))]

windows_resource = rule(
    implementation = _windows_resource_impl,
    doc = "A COFF object holding an executable's manifest and version information.",
    attrs = {
        "executable": attr.string(mandatory = True, doc = "the executable's file name without `.exe`"),
        "product": attr.string(mandatory = True, doc = "the application's name after `Rikka`"),
        "_manifest": attr.label(
            default = Label("//bazel/windows:application.manifest.in"),
            allow_single_file = True,
        ),
        "_script": attr.label(
            default = Label("//bazel/windows:resource.rc.in"),
            allow_single_file = True,
        ),
        "_version": attr.label(default = Label("//bazel/windows:version")),
    },
)

def rikka_application(name, product, **kwargs):
    """A desktop application's executable; on Windows it carries a manifest and version.

    It also declares `<name>_windows_zip`, the application's portable Windows ZIP.

    Args:
        name: target name and the built executable's file name.
        product: the application's name after `Rikka`, such as `Paint`.
        **kwargs: passed to `rikka_binary`.
    """
    resource = name + "_windows_resource"
    windows_resource(
        name = resource,
        executable = name,
        product = product,
        target_compatible_with = [_WINDOWS],
        visibility = ["//visibility:private"],
    )
    rikka_binary(
        name = name,
        compile_data = kwargs.pop("compile_data", []) + select({
            _WINDOWS: [":" + resource],
            "//conditions:default": [],
        }),
        rustc_flags = kwargs.pop("rustc_flags", []) + select({
            _WINDOWS: ["-Clink-arg=$(location :{})".format(resource)],
            "//conditions:default": [],
        }),
        **kwargs
    )
    windows_zip(
        name = name + "_windows_zip",
        out = name + "-windows-x86_64.zip",
        program = ":" + name,
        files = _NOTICES,
        tags = ["manual"],
        target_compatible_with = [_WINDOWS],
    )

_NOTICES = {
    Label("//:LICENSE"): "licenses/rikka/LICENSE",
    Label("//:crates/graphics/THIRD_PARTY.md"): "licenses/rikka/third-party/graphics.md",
    Label("//:crates/media/THIRD_PARTY.md"): "licenses/rikka/third-party/media.md",
    Label("//:crates/model/THIRD_PARTY.md"): "licenses/rikka/third-party/model.md",
    Label("//:crates/physics/THIRD_PARTY.md"): "licenses/rikka/third-party/physics.md",
    Label("//:crates/text/THIRD_PARTY.md"): "licenses/rikka/third-party/text.md",
    Label("//crates/physics/rikka-physics-fluid:THIRD_PARTY.md"): "licenses/rikka/third-party/physics-fluid.md",
}

WindowsRuntimeInfo = provider(
    doc = "Parts of the Windows runtime that a library loads or embeds.",
    fields = {
        "files": "depset of `FILE=ENTRY` placements",
        "trees": "depset of `DIRECTORY=ENTRY` placements",
    },
)

def _windows_runtime_impl(ctx):
    runtime = NIX["windows_runtime"]
    files = ["{}/bin/{}={}".format(runtime, library, library) for library in ctx.attr.libraries]
    files += [
        "{}/{}={}".format(NIX["icon_fonts"], name, entry)
        for name, entry in ctx.attr.icon_font_licenses.items()
    ]
    trees = ["{}/share/licenses/{}=licenses/{}".format(runtime, name, name) for name in ctx.attr.licenses]
    return [WindowsRuntimeInfo(files = depset(files), trees = depset(trees))]

windows_runtime = rule(
    implementation = _windows_runtime_impl,
    doc = """What a library needs beside the program on Windows, from `NIX["windows_runtime"]`.

    A library lists it in its `data`; every application's Windows ZIP that reaches the library
    ships these DLLs and licences.
    """,
    attrs = {
        "icon_font_licenses": attr.string_dict(doc = "licence files of the icon fonts and their entries"),
        "libraries": attr.string_list(doc = "DLLs in the runtime's `bin`, placed at the archive root"),
        "licenses": attr.string_list(doc = "folders in the runtime's `share/licenses`, placed in `licenses/`"),
    },
)

_ShippedInfo = provider(
    doc = "The Windows runtime parts a target and its dependencies need.",
    fields = {
        "files": "depset of `FILE=ENTRY` placements",
        "trees": "depset of `DIRECTORY=ENTRY` placements",
    },
)

_SHIPPED_ATTRIBUTES = ["deps", "data"]

def _shipped_impl(target, ctx):
    files = []
    trees = []
    if WindowsRuntimeInfo in target:
        files.append(target[WindowsRuntimeInfo].files)
        trees.append(target[WindowsRuntimeInfo].trees)
    for attribute in _SHIPPED_ATTRIBUTES:
        dependencies = getattr(ctx.rule.attr, attribute, [])
        if type(dependencies) != "list":
            continue
        for dependency in dependencies:
            if _ShippedInfo in dependency:
                files.append(dependency[_ShippedInfo].files)
                trees.append(dependency[_ShippedInfo].trees)
    return [_ShippedInfo(files = depset(transitive = files), trees = depset(transitive = trees))]

_shipped = aspect(
    implementation = _shipped_impl,
    attr_aspects = _SHIPPED_ATTRIBUTES,
)

def _windows_zip_impl(ctx):
    executable = ctx.attr.program[DefaultInfo].files_to_run.executable
    shipped = ctx.attr.program[_ShippedInfo]
    args = ctx.actions.args()
    args.add("--objdump", NIX["windows"]["objdump"])
    args.add("--output", ctx.outputs.out)
    args.add("--program", "{}={}".format(executable.path, executable.basename))
    args.add_all(shipped.files, before_each = "--file")
    args.add_all(shipped.trees, before_each = "--tree")
    files = []
    for target, entry in ctx.attr.files.items():
        file = target.files.to_list()
        if len(file) != 1:
            fail("{} must be exactly one file".format(target.label))
        files.append(file[0])
        args.add("--file", "{}={}".format(file[0].path, entry))
    ctx.actions.run(
        executable = ctx.executable._package,
        arguments = [args],
        inputs = [executable] + files,
        outputs = [ctx.outputs.out],
        mnemonic = "WindowsZip",
        progress_message = "Packing %{output}",
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

windows_zip = rule(
    implementation = _windows_zip_impl,
    doc = """One application's portable Windows distribution as a deterministic ZIP.

    The program and the DLLs its libraries load sit at the archive root, the fonts are compiled
    into the program, and the licences are in `licenses/`. Packing fails when the program or a DLL
    imports a library that neither Windows nor the archive provides.
    """,
    attrs = {
        "files": attr.label_keyed_string_dict(allow_files = True, doc = "more files and their entries"),
        "out": attr.output(mandatory = True),
        "program": attr.label(
            mandatory = True,
            aspects = [_shipped],
            doc = "the executable placed at the archive root",
        ),
        "_package": attr.label(
            default = Label("//xtask/windows/package:windows-package"),
            executable = True,
            cfg = "exec",
        ),
    },
)
