"""Exposes the development shell's Nix toolchain to Bazel as the `@nix` repository."""

_TOOLCHAIN = "RIKKA_BAZEL_TOOLCHAIN"
_LANGUAGES = "RIKKA_BAZEL_LANGUAGES"
_LOCAL = "RIKKA_BAZEL_LOCAL"
_IMAGE = "RIKKA_RBE_IMAGE"
_LANGUAGES_IMAGE = "RIKKA_RBE_LANGUAGES_IMAGE"

_COMPILERS = ["rustc", "rustdoc", "clippy-driver"]

_TOOLS = ["cargo", "rustfmt"]

_WRAPPER = """#!{bash}
exec {target} "$@"
"""

_COMPILER_WRAPPER = """#!{bash}
arguments=()
skip=
for argument in "$@"; do
    if [ -n "$skip" ]; then
        skip=
        continue
    fi
    case $argument in
        --sysroot) skip=1 ;;
        --sysroot=*) ;;
        *) arguments+=("$argument") ;;
    esac
done
exec {target} "${{arguments[@]}}"
"""

def _require(rctx, name):
    value = rctx.getenv(name)
    if not value:
        fail("{} is unset; run Bazel inside `nix develop`".format(name))
    return value

def _wrap(rctx, template, bash, name, target):
    rctx.file(name, template.format(bash = bash, target = target), executable = True)

def _nix_repository_impl(rctx):
    nix = json.decode(rctx.read(_require(rctx, _TOOLCHAIN)))
    languages = json.decode(rctx.read(_require(rctx, _LANGUAGES)))
    local = json.decode(rctx.read(_require(rctx, _LOCAL)))
    images = {
        "image": _require(rctx, _IMAGE),
        "languages_image": _require(rctx, _LANGUAGES_IMAGE),
    }
    bash = nix["bash"]
    analyzer = local["rust_analyzer"]

    for tool in _COMPILERS:
        _wrap(rctx, _COMPILER_WRAPPER, bash, "rust/bin/" + tool, "{}/bin/{}".format(nix["rust"], tool))
    for tool in _TOOLS:
        _wrap(rctx, _WRAPPER, bash, "rust/bin/" + tool, "{}/bin/{}".format(nix["rust"], tool))
    _wrap(rctx, _WRAPPER, bash, "rust/bin/rust-analyzer", "{}/bin/rust-analyzer".format(analyzer))
    _wrap(
        rctx,
        _WRAPPER,
        bash,
        "rust/bin/rust-analyzer-proc-macro-srv",
        "{}/libexec/rust-analyzer-proc-macro-srv".format(analyzer),
    )
    rctx.symlink("{}/lib/rustlib/src/rust".format(analyzer), "rust/src")
    rctx.file("rust/host_tools", "")

    rctx.file("paths.bzl", "NIX = {}\n".format(json.encode_indent(nix | languages | local | images, indent = "    ")))
    rctx.template(
        "BUILD.bazel",
        Label("//bazel/nix:BUILD.nix.bazel"),
        substitutions = {"{toolchains}": str(Label("//bazel/nix:toolchains.bzl"))},
    )

nix_repository = repository_rule(implementation = _nix_repository_impl)

def _nix_impl(mctx):
    nix_repository(name = "nix")
    return mctx.extension_metadata(
        root_module_direct_deps = ["nix"],
        root_module_direct_dev_deps = [],
    )

nix = module_extension(implementation = _nix_impl)
