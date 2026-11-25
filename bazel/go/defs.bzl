"""Rikka's Go rules: cgo packages built by the development shell's Go through pkg-config."""

load("@nix//:paths.bzl", "NIX")
load("//bazel/platforms:defs.bzl", "LANGUAGES")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "hermetic_test")

RikkaGoInfo = provider(
    doc = "A Go module that links a C library through `#cgo pkg-config`.",
    fields = {
        "files": "the module's files, go.mod included",
        "headers": "headers of the C library it links",
        "include": "the include directory those headers live under",
        "library": "the shared library it links",
        "pkgconfig": "the pkg-config name the module asks for",
    },
)

def _library_impl(ctx):
    library = ctx.file.library
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        RikkaGoInfo(
            files = ctx.files.srcs,
            headers = ctx.files.headers,
            include = ctx.attr.include,
            library = library,
            pkgconfig = ctx.attr.pkgconfig,
        ),
    ]

rikka_go_library = rule(
    implementation = _library_impl,
    exec_compatible_with = LANGUAGES,
    doc = "A Go module whose root is this package, linking a C library through pkg-config.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".go", ".mod"], mandatory = True),
        "headers": attr.label_list(allow_files = [".h"], doc = "Headers of the linked library."),
        "include": attr.string(mandatory = True, doc = "Workspace-relative include directory."),
        "library": attr.label(allow_single_file = True, mandatory = True, doc = "The versioned shared library."),
        "pkgconfig": attr.string(mandatory = True, doc = "The name in `#cgo pkg-config`."),
    },
)

_BUILD = """set -eu
export PATH="{path}"
root="$PWD"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
module="$scratch/module"
mkdir -p "$module/{package}" "$scratch/lib" "$scratch/pkgconfig"
for file in {sources}; do cp "$file" "$module/"; done
for file in {programs}; do cp "$file" "$module/{package}/"; done
ln -s "$root/{library}" "$scratch/lib/{soname}"
ln -s "{soname}" "$scratch/lib/lib{link}.so"
cat > "$scratch/pkgconfig/{pkgconfig}.pc" <<EOF
Name: {pkgconfig}
Description: {pkgconfig} for Go programs built by Bazel
Version: 0
Libs: -L$scratch/lib -l{link}
Cflags: -I$root/{include}
EOF
export HOME="$scratch/home" GOPATH="$scratch/gopath" GOCACHE="$scratch/cache" GOENV=off
export GOROOT="{goroot}" GOTOOLCHAIN=local GOPROXY=off GOFLAGS=-mod=mod GOTELEMETRY=off
export CGO_ENABLED=1 CC="{cc}" PKG_CONFIG_PATH="$scratch/pkgconfig"
cd "$module"
"$GOROOT/bin/go" vet ./...
"$GOROOT/bin/go" {command} -trimpath -o "$root/{output}" ./{package}
"""

_LAUNCHER = """#!{bash}
set -eu
export LD_LIBRARY_PATH="$PWD/{libraries}"
exec "$PWD/{binary}" {arguments}"$@"
"""

def _program_impl(ctx, build):
    library = ctx.attr.library[RikkaGoInfo]
    binary = ctx.actions.declare_file(ctx.label.name + build.suffix)
    shared = library.library
    ctx.actions.run(
        executable = NIX["bash"],
        outputs = [binary],
        inputs = library.files + library.headers + [shared] + ctx.files.srcs,
        arguments = ["-c", _BUILD.format(
            sources = " ".join([file.path for file in library.files]),
            programs = " ".join([file.path for file in ctx.files.srcs]),
            package = build.package,
            command = build.command,
            library = shared.path,
            soname = shared.basename,
            link = shared.basename.removeprefix("lib").partition(".so")[0],
            pkgconfig = library.pkgconfig,
            include = library.include,
            goroot = NIX["go"],
            cc = NIX["gnu"]["cc"],
            path = NIX["language_path"],
            output = binary.path,
        )],
        mnemonic = "GoCompile",
        progress_message = "Compiling Go %{label}",
        use_default_shell_env = False,
    )
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        launcher,
        _LAUNCHER.format(
            bash = NIX["bash"],
            libraries = shared.short_path.rpartition("/")[0],
            binary = binary.short_path,
            arguments = build.arguments,
        ),
        is_executable = True,
    )
    return [
        DefaultInfo(executable = launcher, runfiles = ctx.runfiles(files = [binary, shared])),
        RunEnvironmentInfo(environment = SHIPPED_ENV | ctx.attr.env),
    ]

def _binary_impl(ctx):
    return _program_impl(ctx, struct(
        package = "cmd/" + ctx.label.name,
        command = "build",
        suffix = ".bin",
        arguments = "",
    ))

def _test_impl(ctx):
    return _program_impl(ctx, struct(
        package = "",
        command = "test -c",
        suffix = ".test",
        arguments = "-test.v -test.count=1 ",
    ))

_PROGRAM_ATTRS = {
    "library": attr.label(providers = [RikkaGoInfo], mandatory = True),
    "env": attr.string_dict(),
}

rikka_go_binary = rule(
    implementation = _binary_impl,
    exec_compatible_with = LANGUAGES,
    executable = True,
    doc = "A Go command (`package main`) over a `rikka_go_library`, vetted, built inside its module and run from the runfiles root.",
    attrs = _PROGRAM_ATTRS | {
        "srcs": attr.label_list(allow_files = [".go"], mandatory = True, doc = "The command's `package main` files."),
    },
)

_rikka_go_test_program = rule(
    implementation = _test_impl,
    exec_compatible_with = LANGUAGES,
    executable = True,
    attrs = _PROGRAM_ATTRS | {
        "srcs": attr.label_list(allow_files = [".go"], mandatory = True),
    },
)

def rikka_go_test(name, srcs, library, env = {}, tags = [], timeout = "short"):
    """External Go tests of a `rikka_go_library`, vetted, built and run hermetically.

    `<name>` is the hermetic launcher of the `<name>_bin` test program.

    Args:
        name: target name.
        srcs: `_test.go` files of the module's external test package.
        library: the `rikka_go_library` under test.
        env: environment of the test program.
        tags: tags of both targets.
        timeout: Bazel's timeout for the program.
    """
    _rikka_go_test_program(
        name = name + "_bin",
        srcs = srcs,
        library = library,
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
