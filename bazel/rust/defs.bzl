"""Rikka's Rust rules: the project's edition, lints, profiles and RUNPATH on every crate."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_doc", "rust_library", "rust_proc_macro", "rust_shared_library", "rust_test")
load("//bazel/config:linkage.bzl", "linkage")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "LIBTEST", "PROGRAM", "hermetic_test")

_VERSION = "0.1.0"

_EDITION = "2024"

_LINTS = Label("//bazel/rust:lints")

_DEVELOPMENT = [
    "-Copt-level=1",
    "-Cdebuginfo=line-tables-only",
    "-Cdebug-assertions=on",
    "-Coverflow-checks=on",
    "-Zshare-generics=y",
    "-Zmir-move-elimination=yes",
]

_CRANELIFT = "-Zcodegen-backend=cranelift"

_LINK_ARG = "-Clink-arg="

_LOCALES = "RIKKA_LOCALES"

_GPU_TAGS = ["gpu", "resources:gpu:1"]

_MACHINE_TAGS = ["machine", "exclusive"]

_TESTED_ON = ["@platforms//os:linux"]

_WRAPPED = "wrapped"

_TEST_JOBS = 4

_MAX_SHARDS = 8

_TOPICS_PER_SHARD = 3

_NON_TOPICS = [
    "tests/main.rs",
    "tests/machine.rs",
    "tests/common.rs",
    "tests/common/mod.rs",
    "tests/support.rs",
    "tests/support/mod.rs",
]

def _profile(cranelift):
    development = select({
        Label("//bazel/config:fastbuild"): _DEVELOPMENT,
        "//conditions:default": [],
    })
    if not cranelift:
        return development
    return development + select({
        Label("//bazel/config:cranelift"): [_CRANELIFT],
        "//conditions:default": [],
    })

def _layout(kwargs, folder, crate_root):
    layout = {}
    if "srcs" not in kwargs:
        layout["srcs"] = native.glob([folder + "/**/*.rs"])
    if "crate_root" not in kwargs:
        layout["crate_root"] = crate_root
    return kwargs | layout

def _crate(kwargs, cranelift, link = None):
    rustc_flags = kwargs.pop("rustc_flags", []) + _profile(cranelift)
    if link != None:
        rustc_flags = rustc_flags + link
    rustc_env = dict(kwargs.pop("rustc_env", {}))
    compile_data = kwargs.pop("compile_data", [])
    locales = native.glob(["locales/*.toml"], allow_empty = True)
    if locales:
        rustc_env[_LOCALES] = " ".join(sorted([locale.rpartition("/")[2] for locale in locales]))
        compile_data = compile_data + locales
    return kwargs | {
        "compile_data": compile_data,
        "edition": _EDITION,
        "lint_config": _LINTS,
        "rustc_env": rustc_env,
        "rustc_flags": rustc_flags,
        "version": _VERSION,
    }

def rikka_library(name, cranelift = True, **kwargs):
    """A Rust library compiled with the project's settings.

    Its sources are `src/**/*.rs` with `src/lib.rs` as the crate root unless `srcs` or
    `crate_root` say otherwise. A public library also gets `<name>_doc`, its documentation
    built with warnings denied.

    Args:
        name: target name; its crate name replaces `-` with `_` unless `crate_name` says otherwise.
        cranelift: whether development builds use the Cranelift backend.
        **kwargs: passed to `rust_library`.
    """
    public = "//visibility:public" in kwargs.get("visibility", [])
    kwargs = _layout(kwargs, "src", "src/lib.rs")
    rust_library(name = name, **_crate(kwargs, cranelift))
    if public:
        rust_doc(
            name = name + "_doc",
            crate = ":" + name,
            rustdoc_flags = ["-Dwarnings"],
            tags = ["manual"],
            testonly = kwargs.get("testonly", False),
        )

def rikka_proc_macro(name, **kwargs):
    """A procedural macro crate compiled with the project's settings.

    Its sources are `src/**/*.rs` with `src/lib.rs` as the crate root unless named.

    Args:
        name: target name.
        **kwargs: passed to `rust_proc_macro`.
    """
    kwargs = _layout(kwargs, "src", "src/lib.rs")
    rust_proc_macro(name = name, **_crate(kwargs, cranelift = False))

def rikka_binary(name, cranelift = True, **kwargs):
    """An executable linked with the session's RUNPATH.

    Its sources are `src/**/*.rs` with `src/main.rs` as the crate root unless named.

    Args:
        name: target name and the built executable's file name.
        cranelift: whether development builds use the Cranelift backend.
        **kwargs: passed to `rust_binary`.
    """
    kwargs = _layout(kwargs, "src", "src/main.rs")
    kwargs["env"] = SHIPPED_ENV | kwargs.get("env", {})
    rust_binary(name = name, **_crate(kwargs, cranelift, linkage(executable = True, prefix = _LINK_ARG)))

def rikka_shared_library(name, soname, cranelift = True, **kwargs):
    """A C-ABI shared library linked with the session's RUNPATH.

    Its sources are `src/**/*.rs` with `src/lib.rs` as the crate root unless named.

    Args:
        name: target name; the crate name replaces `-` with `_` unless `crate_name` says otherwise.
        soname: the `DT_SONAME` the library is loaded by, such as `librikka.so.0`.
        cranelift: whether development builds use the Cranelift backend.
        **kwargs: passed to `rust_shared_library`.
    """
    rustc_flags = kwargs.pop("rustc_flags", []) + ["-Clink-arg=-Wl,-soname," + soname]
    kwargs = _layout(kwargs | {"rustc_flags": rustc_flags}, "src", "src/lib.rs")
    rust_shared_library(name = name, **_crate(kwargs, cranelift, linkage(executable = False, prefix = _LINK_ARG)))

def rikka_test(name, gpu = False, machine = False, shared = False, limit = 60, **kwargs):
    """A crate's test binary, run hermetically and remotely unless it needs this machine.

    Its sources are `tests/**/*.rs` with `tests/<name>.rs` as the crate root, and every other
    file under `tests/` is its data, unless `srcs`, `crate_root` or `data` say otherwise.
    `<name>` is the hermetic launcher; it runs every test of the `<name>_bin` binary in its own
    process and scratch tree with `limit` seconds, several at once unless the tests need this
    machine, spread over Bazel shards; topic files set the shard count, and a shard may take
    Bazel's `long` timeout.

    Args:
        name: target name.
        gpu: the tests need this machine's GPU; run them with `--config=gpu`.
        machine: the tests need input devices, a sound card or the session; run them with `--config=machine`.
        shared: run every test in one process, one shard.
        limit: seconds each test may take.
        **kwargs: passed to `rust_test`.
    """
    kwargs = _layout(kwargs, "tests", "tests/" + name + ".rs")
    if "data" not in kwargs:
        kwargs = kwargs | {"data": native.glob(["tests/**"], allow_empty = True, exclude = ["tests/**/*.rs"])}
    tags = kwargs.pop("tags", [])
    visibility = kwargs.pop("visibility", None)
    launcher_tags = tags + (_GPU_TAGS if gpu else []) + (_MACHINE_TAGS if machine else [])
    jobs = 1 if gpu or machine or shared else _TEST_JOBS
    concurrency = {}
    if jobs > 1:
        launcher_tags = launcher_tags + ["cpu:%d" % jobs]
        concurrency = {"EstimatedCPU": str(jobs)}
    topics = len(native.glob(
        ["tests/*.rs", "tests/*/mod.rs"],
        allow_empty = True,
        exclude = _NON_TOPICS,
    ))
    rust_test(
        name = name + "_bin",
        tags = tags + [_WRAPPED],
        target_compatible_with = _TESTED_ON,
        **_crate(kwargs, cranelift = False, link = linkage(executable = True, prefix = _LINK_ARG))
    )
    hermetic_test(
        name = name,
        binary = ":" + name + "_bin",
        exec_properties = concurrency,
        jobs = jobs,
        limit = limit,
        runner = PROGRAM if shared else LIBTEST,
        shard_count = 1 if shared or machine else min(_MAX_SHARDS, max(1, topics // _TOPICS_PER_SHARD)),
        tags = launcher_tags,
        timeout = "long",
        visibility = visibility,
    )
