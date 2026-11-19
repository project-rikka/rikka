"""Exposes the development shell's Nix toolchain to Bazel as the `@nix` repository."""

_TOOLCHAIN = "RIKKA_BAZEL_TOOLCHAIN"
_LOCAL = "RIKKA_BAZEL_LOCAL"
_IMAGE = "RIKKA_RBE_IMAGE"

_COMPILERS = ["rustc", "rustdoc", "clippy-driver"]

_TOOLS = ["cargo", "rustfmt", "rust-analyzer"]

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
    local = json.decode(rctx.read(_require(rctx, _LOCAL)))
    image = _require(rctx, _IMAGE)
    bash = nix["bash"]

    for tool in _COMPILERS:
        _wrap(rctx, _COMPILER_WRAPPER, bash, "rust/bin/" + tool, "{}/bin/{}".format(nix["rust"], tool))
    for tool in _TOOLS:
        _wrap(rctx, _WRAPPER, bash, "rust/bin/" + tool, "{}/bin/{}".format(nix["rust"], tool))
    _wrap(
        rctx,
        _WRAPPER,
        bash,
        "rust/bin/rust-analyzer-proc-macro-srv",
        "{}/libexec/rust-analyzer-proc-macro-srv".format(nix["rust"]),
    )
    rctx.symlink("{}/lib/rustlib/src/rust".format(nix["rust"]), "rust/src")
    rctx.file("rust/host_tools", "")

    rctx.file("paths.bzl", "NIX = {}\n".format(json.encode_indent(nix | local | {"image": image}, indent = "    ")))
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
