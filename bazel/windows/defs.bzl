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

def _windows_zip_impl(ctx):
    runtime = NIX["windows_runtime"]
    icon_fonts = NIX["icon_fonts"]
    args = ctx.actions.args()
    args.add("--objdump", NIX["windows"]["objdump"])
    args.add("--output", ctx.outputs.out)
    programs = []
    for program in ctx.attr.programs:
        executable = program[DefaultInfo].files_to_run.executable
        programs.append(executable)
        args.add("--program", "{}={}".format(executable.path, executable.basename))
    args.add("--tree", runtime + "/bin=")
    args.add("--tree", runtime + "/share/licenses=licenses")
    for name, entry in ctx.attr.icon_font_licenses.items():
        args.add("--file", "{}/{}={}".format(icon_fonts, name, entry))
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
        inputs = programs + files,
        outputs = [ctx.outputs.out],
        mnemonic = "WindowsZip",
        progress_message = "Packing %{output}",
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

windows_zip = rule(
    implementation = _windows_zip_impl,
    doc = """The portable Windows distribution as a deterministic ZIP.

    The programs and the DLLs they load sit at the archive root, the fonts are compiled into the
    programs, and the licences are in `licenses/`. Packing fails when a program or DLL imports a
    library that neither Windows nor the archive provides.
    """,
    attrs = {
        "files": attr.label_keyed_string_dict(allow_files = True, doc = "more files and their entries"),
        "icon_font_licenses": attr.string_dict(doc = "licence files of the icon fonts and their entries"),
        "out": attr.output(mandatory = True),
        "programs": attr.label_list(mandatory = True, doc = "the executables placed at the archive root"),
        "_package": attr.label(
            default = Label("//xtask/windows/package:windows-package"),
            executable = True,
            cfg = "exec",
        ),
    },
)
