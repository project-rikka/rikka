"""Rikka's Astro rules: sites built, checked and served by the development shell's Node."""

load("@nix//:paths.bzl", "NIX")
load("//bazel/test:defs.bzl", "hermetic_test")

_ENVIRONMENT = """export PATH="{path}"
export ASTRO_TELEMETRY_DISABLED=1
export FONTCONFIG_FILE="{fontconfig_file}"
""".format(
    path = NIX["path"],
    fontconfig_file = NIX["fontconfig_file"],
)

_STAGE = """stage() {{
    local from="$1" into="$2" manifest="$3"
    while IFS= read -r file; do
        mkdir -p "$into/$(dirname "$file")"
        cp "$from/$file" "$into/$file"
    done < "$manifest"
    ln -s "{modules}" "$into/{package}/node_modules"
}}
"""

_BUILD = """set -eu
{environment}
root="$PWD"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
{stage}
stage "$root" "$scratch/tree" "$root/{manifest}"
export HOME="$scratch/home"
cd "$scratch/tree/{package}"
"{node}" node_modules/.bin/astro build --outDir "$scratch/dist"
cp -R "$scratch/dist/." "$root/{output}/"
"""

_TEST = """#!{bash}
set -eu
{environment}
{stage}
stage "$PWD" "$TMPDIR/tree" "$PWD/{manifest}"
cd "$TMPDIR/tree/{package}"
exec "{node}" node_modules/.bin/{tool} {arguments} "$@"
"""

_RUN = """#!{bash}
set -eu
{environment}
package="$BUILD_WORKSPACE_DIRECTORY/{package}"
if [ -e "$package/node_modules" ] && [ ! -L "$package/node_modules" ]; then
    echo "{package}/node_modules is not a link to the Nix store; remove it" >&2
    exit 1
fi
ln -sfn "{modules}" "$package/node_modules"
cd "$package"
exec "{node}" node_modules/.bin/{tool} {arguments} "$@"
"""

def _sources(ctx):
    files = ctx.files.srcs + ctx.files.data
    for file in files:
        if not file.is_source:
            fail("{} is generated; Astro sites stage source files only".format(file.short_path))
    manifest = ctx.actions.declare_file(ctx.label.name + ".manifest")
    ctx.actions.write(manifest, "".join([file.short_path + "\n" for file in files]))
    return manifest, files

def _stage(ctx):
    return _STAGE.format(
        modules = NIX["website_modules"],
        package = ctx.label.package,
    )

def _quote(arguments):
    return " ".join(["'{}'".format(argument.replace("'", "'\\''")) for argument in arguments])

def _site_impl(ctx):
    manifest, files = _sources(ctx)
    output = ctx.actions.declare_directory(ctx.label.name)
    ctx.actions.run(
        executable = NIX["bash"],
        outputs = [output],
        inputs = files + [manifest],
        arguments = ["-c", _BUILD.format(
            environment = _ENVIRONMENT,
            stage = _stage(ctx),
            manifest = manifest.path,
            package = ctx.label.package,
            node = NIX["node"],
            output = output.path,
        )],
        mnemonic = "AstroBuild",
        progress_message = "Building site %{label}",
        use_default_shell_env = False,
    )
    return [DefaultInfo(files = depset([output]))]

_SOURCE_ATTRS = {
    "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "The site's own files: config, sources, public assets, package.json."),
    "data": attr.label_list(allow_files = True, doc = "Files elsewhere in the workspace the site reads, staged at their workspace paths."),
}

rikka_astro_site = rule(
    implementation = _site_impl,
    doc = """A static Astro site built from this package into a directory.

    The sources and data are staged at their workspace paths with the Nix `node_modules` linked into
    the package, so the site reads workspace files by relative path exactly as `rikka_astro_run` does.
    """,
    attrs = _SOURCE_ATTRS,
)

def _test_program_impl(ctx):
    manifest, files = _sources(ctx)
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        launcher,
        _TEST.format(
            bash = NIX["bash"],
            environment = _ENVIRONMENT,
            stage = _stage(ctx),
            manifest = manifest.short_path,
            package = ctx.label.package,
            node = NIX["node"],
            tool = ctx.attr.tool,
            arguments = _quote(ctx.attr.arguments),
        ),
        is_executable = True,
    )
    return [DefaultInfo(executable = launcher, runfiles = ctx.runfiles(files = files + [manifest]))]

_TOOL_ATTRS = {
    "tool": attr.string(mandatory = True, doc = "A command of `node_modules/.bin`."),
    "arguments": attr.string_list(doc = "Arguments given before any of the caller's."),
}

_rikka_astro_test_program = rule(
    implementation = _test_program_impl,
    executable = True,
    attrs = _SOURCE_ATTRS | _TOOL_ATTRS,
)

def rikka_astro_test(name, srcs, tool, arguments = [], data = [], tags = [], timeout = "short"):
    """Runs a `node_modules/.bin` tool over a staged copy of an Astro site, hermetically.

    `<name>` is the hermetic launcher of the `<name>_bin` program.

    Args:
        name: target name.
        srcs: the site's own files.
        tool: the command of `node_modules/.bin` to run.
        arguments: its arguments.
        data: workspace files the site reads.
        tags: tags of both targets.
        timeout: Bazel's timeout for the program.
    """
    _rikka_astro_test_program(
        name = name + "_bin",
        srcs = srcs,
        data = data,
        tool = tool,
        arguments = arguments,
        tags = tags,
        testonly = True,
    )
    hermetic_test(
        name = name,
        binary = ":" + name + "_bin",
        tags = tags,
        timeout = timeout,
    )

def _run_impl(ctx):
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        launcher,
        _RUN.format(
            bash = NIX["bash"],
            environment = _ENVIRONMENT,
            package = ctx.label.package,
            modules = NIX["website_modules"],
            node = NIX["node"],
            tool = ctx.attr.tool,
            arguments = _quote(ctx.attr.arguments),
        ),
        is_executable = True,
    )
    return [DefaultInfo(executable = launcher)]

rikka_astro_run = rule(
    implementation = _run_impl,
    executable = True,
    doc = """Runs a `node_modules/.bin` tool in this package of the workspace itself.

    It links the Nix `node_modules` into the package first, so editors and the tool see the same
    modules the build stages.
    """,
    attrs = _TOOL_ATTRS,
)
