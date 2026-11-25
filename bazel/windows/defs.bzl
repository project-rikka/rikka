"""Windows executables, libraries and the portable Windows distribution."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@nix//:paths.bzl", "NIX")
load("//bazel/rust:defs.bzl", "rikka_binary", "rikka_shared_library")

_WINDOWS = "@platforms//os:windows"

_WINDOWS_PLATFORM = Label("//bazel/platforms:windows")

_PRODUCT = "Rikka"

_EXECUTABLE_TYPE = "0x1"

_LIBRARY_TYPE = "0x2"

_LIBRARY_EXTENSION = ".dll"

_IMPORT_LIBRARY_EXTENSION = ".dll.lib"

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
    inputs = []
    manifest_line = ""
    if not ctx.attr.library:
        manifest = ctx.actions.declare_file(ctx.attr.file_name + ".manifest")
        ctx.actions.expand_template(
            template = ctx.file._manifest,
            output = manifest,
            substitutions = {
                "@DESCRIPTION@": _xml_text(description),
                "@IDENTITY@": "{}.{}".format(_PRODUCT, ctx.attr.product.replace(" ", "")),
                "@VERSION@": ".".join(numbers),
            },
        )
        inputs.append(manifest)
        manifest_line = "1 24 \"{}\"\n".format(manifest.basename)

    script = ctx.actions.declare_file(ctx.attr.file_name + ".rc")
    ctx.actions.expand_template(
        template = ctx.file._script,
        output = script,
        substitutions = {
            "@DESCRIPTION@": _rc_string(description),
            "@FILE_NAME@": _rc_string(ctx.attr.file_name),
            "@FILE_TYPE@": _LIBRARY_TYPE if ctx.attr.library else _EXECUTABLE_TYPE,
            "@INTERNAL_NAME@": _rc_string(paths.split_extension(ctx.attr.file_name)[0]),
            "@MANIFEST@": manifest_line,
            "@VERSION@": version,
            "@VERSION_COMMAS@": ",".join(numbers),
        },
    )

    resource = ctx.actions.declare_file(ctx.attr.file_name + ".res.o")
    ctx.actions.run(
        executable = NIX["windows"]["windres"],
        arguments = [
            "--input-format=rc",
            "--output-format=coff",
            "--include-dir=" + paths.dirname(script.path),
            "--input=" + script.path,
            "--output=" + resource.path,
        ],
        inputs = [script] + inputs,
        outputs = [resource],
        mnemonic = "WindowsResource",
        progress_message = "Compiling the Windows resources of %{label}",
    )
    return [DefaultInfo(files = depset([resource]))]

windows_resource = rule(
    implementation = _windows_resource_impl,
    doc = "A COFF object holding an executable's manifest and version information, or a library's version.",
    attrs = {
        "file_name": attr.string(mandatory = True, doc = "the built file's name, such as `rikka-paint.exe`"),
        "library": attr.bool(doc = "the file is a DLL, which carries no application manifest"),
        "product": attr.string(mandatory = True, doc = "the product's name after `Rikka`"),
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

def _linked_resource(name, file_name, product, library, kwargs):
    resource = name + "_windows_resource"
    windows_resource(
        name = resource,
        file_name = file_name,
        library = library,
        product = product,
        target_compatible_with = [_WINDOWS],
        visibility = ["//visibility:private"],
    )
    return kwargs | {
        "compile_data": kwargs.pop("compile_data", []) + select({
            _WINDOWS: [":" + resource],
            "//conditions:default": [],
        }),
        "rustc_flags": kwargs.pop("rustc_flags", []) + select({
            _WINDOWS: ["-Clink-arg=$(location :{})".format(resource)],
            "//conditions:default": [],
        }),
    }

def rikka_application(name, product, **kwargs):
    """A desktop application's executable; on Windows it carries a manifest and version.

    It also declares `<name>_windows_zip`, the application's portable Windows ZIP.

    Args:
        name: target name and the built executable's file name.
        product: the application's name after `Rikka`, such as `Paint`.
        **kwargs: passed to `rikka_binary`.
    """
    rikka_binary(name = name, **_linked_resource(name, name + ".exe", product, False, kwargs))
    windows_zip(
        name = name + "_windows_zip",
        out = name + "-windows-x86_64.zip",
        program = ":" + name,
        tags = ["manual"],
    )

def rikka_native_library(name, product, soname, **kwargs):
    """A C-ABI shared library that programs in other languages load; on Windows it carries its version.

    Args:
        name: target name; the crate name replaces `-` with `_` unless `crate_name` says otherwise.
        product: the library's name after `Rikka`, such as `SDK`.
        soname: the ELF `DT_SONAME` the library is loaded by on Linux.
        **kwargs: passed to `rikka_shared_library`.
    """
    file_name = kwargs.get("crate_name", name.replace("-", "_")) + _LIBRARY_EXTENSION
    rikka_shared_library(name = name, soname = soname, **_linked_resource(name, file_name, product, True, kwargs))

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
        "libraries": "depset of DLL paths, placed beside the binary that loads them",
        "files": "depset of `FILE=ENTRY` placements",
        "trees": "depset of `DIRECTORY=ENTRY` placements",
    },
)

def _windows_runtime_impl(ctx):
    runtime = NIX["windows_runtime"]
    libraries = ["{}/bin/{}".format(runtime, library) for library in ctx.attr.libraries]
    files = [
        "{}/{}={}".format(NIX["icon_fonts"], name, entry)
        for name, entry in ctx.attr.icon_font_licenses.items()
    ]
    trees = ["{}/share/licenses/{}=licenses/{}".format(runtime, name, name) for name in ctx.attr.licenses]
    return [WindowsRuntimeInfo(
        libraries = depset(libraries),
        files = depset(files),
        trees = depset(trees),
    )]

windows_runtime = rule(
    implementation = _windows_runtime_impl,
    doc = """What a library needs beside the program on Windows, from `NIX["windows_runtime"]`.

    A library lists it in its `data`; every Windows ZIP that reaches the library ships these DLLs
    beside the binary that reaches it, and their licences.
    """,
    attrs = {
        "icon_font_licenses": attr.string_dict(doc = "licence files of the icon fonts and their entries"),
        "libraries": attr.string_list(doc = "DLLs in the runtime's `bin`, placed beside the binary"),
        "licenses": attr.string_list(doc = "folders in the runtime's `share/licenses`, placed in `licenses/`"),
    },
)

_ShippedInfo = provider(
    doc = "The Windows runtime parts a target and its dependencies need.",
    fields = {
        "libraries": "depset of DLL paths",
        "files": "depset of `FILE=ENTRY` placements",
        "trees": "depset of `DIRECTORY=ENTRY` placements",
    },
)

_SHIPPED_ATTRIBUTES = ["deps", "data"]

def _shipped_impl(target, ctx):
    libraries = []
    files = []
    trees = []
    if WindowsRuntimeInfo in target:
        libraries.append(target[WindowsRuntimeInfo].libraries)
        files.append(target[WindowsRuntimeInfo].files)
        trees.append(target[WindowsRuntimeInfo].trees)
    for attribute in _SHIPPED_ATTRIBUTES:
        dependencies = getattr(ctx.rule.attr, attribute, [])
        if type(dependencies) != "list":
            continue
        for dependency in dependencies:
            if _ShippedInfo in dependency:
                libraries.append(dependency[_ShippedInfo].libraries)
                files.append(dependency[_ShippedInfo].files)
                trees.append(dependency[_ShippedInfo].trees)
    return [_ShippedInfo(
        libraries = depset(transitive = libraries),
        files = depset(transitive = files),
        trees = depset(transitive = trees),
    )]

_shipped = aspect(
    implementation = _shipped_impl,
    attr_aspects = _SHIPPED_ATTRIBUTES,
)

def _to_windows_impl(_settings, _attr):
    return {"//command_line_option:platforms": [str(_WINDOWS_PLATFORM)]}

_to_windows = transition(
    implementation = _to_windows_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _entry(folder, name):
    return paths.join(folder, name) if folder else name

def _one_file(target, extension):
    found = [file for file in target.files.to_list() if file.basename.endswith(extension)]
    if len(found) != 1:
        fail("{} must build exactly one {} file".format(target.label, extension))
    return found[0]

def _windows_zip_impl(ctx):
    if not ctx.attr.program and not ctx.attr.libraries:
        fail("a Windows ZIP needs a program or libraries")
    args = ctx.actions.args()
    args.add("--objdump", NIX["windows"]["objdump"])
    args.add("--output", ctx.outputs.out)
    inputs = []
    shipped = []

    if ctx.attr.program:
        executable = ctx.attr.program[DefaultInfo].files_to_run.executable
        inputs.append(executable)
        args.add("--program", "{}={}".format(executable.path, executable.basename))
        shipped.append(("", ctx.attr.program[_ShippedInfo]))

    for target, folder in ctx.attr.libraries.items():
        library = _one_file(target, _LIBRARY_EXTENSION)
        inputs.append(library)
        args.add("--file", "{}={}".format(library.path, _entry(folder, library.basename)))
        shipped.append((folder, target[_ShippedInfo]))

    for target, entry in ctx.attr.import_libraries.items():
        library = _one_file(target, _IMPORT_LIBRARY_EXTENSION)
        inputs.append(library)
        args.add("--file", "{}={}".format(library.path, entry))

    runtime = {}
    trees = {}
    for folder, info in shipped:
        for library in info.libraries.to_list():
            runtime[_entry(folder, paths.basename(library))] = library
        for placement in info.files.to_list():
            source, _, entry = placement.partition("=")
            runtime[entry] = source
        for placement in info.trees.to_list():
            trees[placement] = None
    for entry, source in sorted(runtime.items()):
        args.add("--file", "{}={}".format(source, entry))
    args.add_all(sorted(trees.keys()), before_each = "--tree")

    for target, entry in (ctx.attr.files | ctx.attr._notices).items():
        file = target.files.to_list()
        if len(file) != 1:
            fail("{} must be exactly one file".format(target.label))
        inputs.append(file[0])
        args.add("--file", "{}={}".format(file[0].path, entry))

    for target, folder in ctx.attr.packages.items():
        prefix = target.label.package + "/"
        for file in target.files.to_list():
            if not file.short_path.startswith(prefix):
                fail("{} is outside {}".format(file.short_path, target.label.package))
            inputs.append(file)
            args.add("--file", "{}={}".format(file.path, _entry(folder, file.short_path[len(prefix):])))

    ctx.actions.run(
        executable = ctx.executable._package,
        arguments = [args],
        inputs = inputs,
        outputs = [ctx.outputs.out],
        mnemonic = "WindowsZip",
        progress_message = "Packing %{output}",
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

windows_zip = rule(
    implementation = _windows_zip_impl,
    cfg = _to_windows,
    doc = """A portable Windows distribution as a deterministic ZIP, built for Windows from any platform.

    The program sits at the archive root and each library in its folder, the DLLs they load beside
    them, and the licences in `licenses/`; fonts are compiled into the binaries. Packing fails when a
    binary imports a library that neither Windows nor its folder provides.
    """,
    attrs = {
        "files": attr.label_keyed_string_dict(allow_files = True, doc = "more files and their entries"),
        "import_libraries": attr.label_keyed_string_dict(
            doc = "shared libraries whose COFF import library is placed at the entry",
        ),
        "libraries": attr.label_keyed_string_dict(
            aspects = [_shipped],
            doc = "shared libraries and the folder each is placed in, empty for the root",
        ),
        "out": attr.output(mandatory = True),
        "packages": attr.label_keyed_string_dict(
            allow_files = True,
            doc = "files placed in a folder under their path within their package",
        ),
        "program": attr.label(
            aspects = [_shipped],
            doc = "the executable placed at the archive root",
        ),
        "_notices": attr.label_keyed_string_dict(allow_files = True, default = _NOTICES),
        "_package": attr.label(
            default = Label("//xtask/windows/package:windows-package"),
            executable = True,
            cfg = "exec",
        ),
    },
)
