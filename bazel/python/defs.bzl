"""Rikka's Python rules: packages and tests run by the development shell's Python."""

load("@nix//:paths.bzl", "NIX")
load("//bazel/platforms:defs.bzl", "LANGUAGES")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "hermetic_test")

RikkaPyInfo = provider(
    doc = "A Python package: its sources, import roots and the shared libraries it loads.",
    fields = {
        "files": "depset of the package's files",
        "imports": "depset of workspace-relative import roots",
        "libraries": "depset of shared libraries the package loads",
    },
)

def _collect(ctx, sources):
    files = depset(sources, transitive = [dep[RikkaPyInfo].files for dep in ctx.attr.deps])
    imports = depset(transitive = [dep[RikkaPyInfo].imports for dep in ctx.attr.deps])
    libraries = depset(transitive = [dep[RikkaPyInfo].libraries for dep in ctx.attr.deps])
    return files, imports, libraries

def _library_impl(ctx):
    files, imports, libraries = _collect(ctx, ctx.files.srcs)
    root = ctx.label.package + ("/" + ctx.attr.root if ctx.attr.root else "")
    return [
        DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = depset(transitive = [files, libraries]))),
        RikkaPyInfo(
            files = files,
            imports = depset([root], transitive = [imports]),
            libraries = depset(ctx.files.libraries, transitive = [libraries]),
        ),
    ]

rikka_py_library = rule(
    implementation = _library_impl,
    exec_compatible_with = LANGUAGES,
    doc = "A pure-Python package importable from `root` within this package.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".py", ".typed"]),
        "root": attr.string(doc = "The import root, relative to this package."),
        "deps": attr.label_list(providers = [RikkaPyInfo]),
        "libraries": attr.label_list(allow_files = True, doc = "Shared libraries loaded at run time."),
    },
)

_LAUNCHER = """#!{bash}
set -eu
export PYTHONPATH="{imports}" MYPYPATH="{imports}"
export LD_LIBRARY_PATH="{libraries}"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONSAFEPATH=1
{command}
"""

def _launch(ctx, sources, command):
    files, imports, libraries = _collect(ctx, sources)
    imports = depset([ctx.label.package], transitive = [imports])
    root = "$PWD/"
    paths = ":".join([root + path for path in imports.to_list()])
    library_dirs = ":".join(sorted({root + library.short_path.rpartition("/")[0]: None for library in libraries.to_list()}.keys()))
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        launcher,
        _LAUNCHER.format(bash = NIX["bash"], imports = paths, libraries = library_dirs, command = command),
        is_executable = True,
    )
    runfiles = ctx.runfiles(files = sources, transitive_files = depset(transitive = [files, libraries]))
    return [
        DefaultInfo(executable = launcher, runfiles = runfiles),
        RunEnvironmentInfo(environment = SHIPPED_ENV | ctx.attr.env),
    ]

def _binary_impl(ctx):
    return _launch(ctx, [ctx.file.main], 'exec {python} "$PWD/{main}" "$@"'.format(
        python = NIX["python"],
        main = ctx.file.main.short_path,
    ))

def _test_impl(ctx):
    python = NIX["python"]
    if ctx.attr.typecheck:
        command = 'exec {python} -m mypy --strict --cache-dir="$TEST_TMPDIR/mypy" --no-error-summary {sources}'.format(
            python = python,
            sources = " ".join([src.short_path for src in ctx.files.srcs]),
        )
    else:
        command = "exec {python} -m unittest --verbose {modules}".format(
            python = python,
            modules = " ".join([src.basename[:-len(".py")] for src in ctx.files.srcs]),
        )
    return _launch(ctx, ctx.files.srcs, command)

rikka_py_binary = rule(
    implementation = _binary_impl,
    exec_compatible_with = LANGUAGES,
    executable = True,
    doc = "A Python script run by the shell's Python from the runfiles root.",
    attrs = {
        "main": attr.label(allow_single_file = [".py"], mandatory = True, doc = "The script that runs."),
        "deps": attr.label_list(providers = [RikkaPyInfo]),
        "env": attr.string_dict(),
    },
)

_rikka_py_test_program = rule(
    implementation = _test_impl,
    exec_compatible_with = LANGUAGES,
    executable = True,
    attrs = {
        "srcs": attr.label_list(allow_files = [".py"]),
        "deps": attr.label_list(providers = [RikkaPyInfo]),
        "typecheck": attr.bool(),
        "env": attr.string_dict(),
    },
)

def rikka_py_test(name, srcs, deps = [], typecheck = False, env = {}, tags = [], timeout = "short"):
    """Python tests run hermetically by the shell's Python as one process.

    `<name>` is the hermetic launcher of the `<name>_bin` program.

    Args:
        name: target name.
        srcs: test modules beside the BUILD file; `unittest` runs each.
        deps: `rikka_py_library` packages the tests import.
        typecheck: check the sources with `mypy --strict` instead of running them.
        env: environment of the program.
        tags: tags of both targets.
        timeout: Bazel's timeout for the program.
    """
    _rikka_py_test_program(
        name = name + "_bin",
        srcs = srcs,
        deps = deps,
        typecheck = typecheck,
        env = env,
        tags = tags,
        testonly = True,
    )
    hermetic_test(
        name = name,
        languages = True,
        binary = ":" + name + "_bin",
        tags = tags,
        timeout = timeout,
    )
