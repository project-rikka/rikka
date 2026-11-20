"""Toolchains over the Nix store paths of the development shell.

Every tool runs from its absolute `/nix/store` path, which the remote executors'
image carries too, so no toolchain file is ever an action input.
"""

load("@rules_cc//cc/private/toolchain:unix_cc_toolchain_config.bzl", "cc_toolchain_config")
load("@rules_cc//cc/toolchains:cc_toolchain.bzl", "cc_toolchain")
load("@rules_rust//cargo:cargo_toolchain.bzl", "cargo_toolchain")
load("@rules_rust//rust:toolchain.bzl", "rust_analyzer_toolchain", "rust_stdlib_filegroup", "rust_toolchain", "rustfmt_toolchain")
load("@rules_shell//shell/toolchains:sh_toolchain.bzl", "sh_toolchain")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

_WINDOWS = ["@platforms//os:windows", "@platforms//cpu:x86_64"]

_LIBC = {
    "gnu": Label("//bazel/platforms:gnu"),
    "musl": Label("//bazel/platforms:musl"),
}

_LINKING = [
    "-Clinker-features=-lld",
    "-Clink-self-contained=-linker",
]

_WINDOWS_STDLIB = ["-lws2_32", "-luserenv", "-lbcrypt", "-lntdll", "-lsynchronization"]

def _rust_environment(nix):
    return {
        "BINDGEN_EXTRA_CLANG_ARGS": nix["bindgen_args"],
        "LIBCLANG_PATH": nix["libclang_path"],
        "LIBRARY_PATH": nix["library_path"],
        "NIX_CFLAGS_COMPILE": nix["cflags"],
        "PATH": nix["path"],
        "PKG_CONFIG_PATH": nix["pkg_config_path"],
        "SOURCE_DATE_EPOCH": "0",
        "VERGEN_IDEMPOTENT": "1",
    }

def _windows_environment(nix):
    return {
        "BINDGEN_EXTRA_CLANG_ARGS": "--target=x86_64-w64-mingw32 --sysroot=" + nix["windows"]["sysroot"],
        "LIBCLANG_PATH": nix["libclang_path"],
        "PATH": nix["path"],
        "SOURCE_DATE_EPOCH": "0",
        "VERGEN_IDEMPOTENT": "1",
    }

def _rust(name, triple, target_compatible_with, env, extra_rustc_flags, stdlib_linkflags, binary_ext = "", dylib_ext = ".so"):
    rust_toolchain(
        name = "rust_{}_toolchain".format(name),
        binary_ext = binary_ext,
        channel = "nightly",
        clippy_driver = "rust/bin/clippy-driver",
        debug_info = {"dbg": "2", "fastbuild": "0", "opt": "0"},
        default_edition = "2024",
        dylib_ext = dylib_ext,
        env = env,
        exec_triple = "x86_64-unknown-linux-gnu",
        extra_exec_rustc_flags = _LINKING + ["-Copt-level=2"],
        extra_rustc_flags = extra_rustc_flags,
        opt_level = {"dbg": "0", "fastbuild": "2", "opt": "3"},
        rust_doc = "rust/bin/rustdoc",
        rust_std = ":rust_std",
        rustc = "rust/bin/rustc",
        rustc_lib = ":empty",
        staticlib_ext = ".a",
        stdlib_linkflags = stdlib_linkflags,
        strip_level = {"dbg": "none", "fastbuild": "none", "opt": "symbols"},
        target_triple = triple,
    )
    native.toolchain(
        name = "rust_{}".format(name),
        exec_compatible_with = _LINUX,
        target_compatible_with = target_compatible_with,
        toolchain = ":rust_{}_toolchain".format(name),
        toolchain_type = "@rules_rust//rust:toolchain_type",
    )

_REPRODUCIBLE = [
    "-no-canonical-prefixes",
    "-Wno-builtin-macro-redefined",
    "-D__DATE__=\"redacted\"",
    "-D__TIMESTAMP__=\"redacted\"",
    "-D__TIME__=\"redacted\"",
]

def _tool_paths(tools):
    return {
        "ar": tools["ar"],
        "cpp": tools["cpp"],
        "gcc": tools["cc"],
        "gcov": "/bin/false",
        "ld": tools["ld"],
        "llvm-cov": "/bin/false",
        "nm": tools["nm"],
        "objcopy": tools["objcopy"],
        "objdump": tools["objdump"],
        "strip": tools["strip"],
    }

def _cc_toolchain(name, target_compatible_with):
    cc_toolchain(
        name = "cc_{}_toolchain".format(name),
        all_files = ":empty",
        ar_files = ":empty",
        as_files = ":empty",
        compiler_files = ":empty",
        dwp_files = ":empty",
        linker_files = ":empty",
        objcopy_files = ":empty",
        strip_files = ":empty",
        supports_param_files = True,
        toolchain_config = ":cc_{}_config".format(name),
    )
    native.toolchain(
        name = "cc_{}".format(name),
        exec_compatible_with = _LINUX,
        target_compatible_with = target_compatible_with,
        toolchain = ":cc_{}_toolchain".format(name),
        toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
    )

def _cc(nix, libc):
    cc_toolchain_config(
        name = "cc_{}_config".format(libc),
        abi_libc_version = libc,
        abi_version = libc,
        compile_flags = ["-fno-omit-frame-pointer"],
        compiler = "gcc",
        cpu = "k8",
        cxx_builtin_include_directories = ["/nix/store"],
        cxx_flags = ["-std=c++17"],
        dbg_compile_flags = ["-g"],
        host_system_name = "x86_64-unknown-linux-gnu",
        link_flags = [
            "-fuse-ld=mold",
            "-B" + nix["mold"],
            "-Wl,-z,pack-relative-relocs",
            "-Wl,--compress-debug-sections=zlib",
        ],
        link_libs = ["-lstdc++", "-lm"],
        opt_compile_flags = ["-O2", "-DNDEBUG", "-ffunction-sections", "-fdata-sections"],
        opt_link_flags = ["-Wl,--gc-sections"],
        supports_start_end_lib = True,
        target_libc = libc,
        target_system_name = "x86_64-unknown-linux-" + libc,
        tool_paths = _tool_paths(nix[libc]),
        toolchain_identifier = "nix-gcc-" + libc,
        unfiltered_compile_flags = _REPRODUCIBLE + ["-fno-canonical-system-headers"],
    )
    _cc_toolchain(libc, _LINUX + [_LIBC[libc]])

def _cc_windows(nix):
    cc_toolchain_config(
        name = "cc_windows_config",
        abi_libc_version = "ucrt",
        abi_version = "mingw",
        compile_flags = ["-fno-omit-frame-pointer"],
        compiler = "clang",
        cpu = "x64_windows",
        cxx_builtin_include_directories = ["/nix/store"],
        cxx_flags = ["-std=c++17"],
        dbg_compile_flags = ["-g"],
        host_system_name = "x86_64-unknown-linux-gnu",
        link_flags = ["-static"],
        link_libs = [],
        opt_compile_flags = ["-O2", "-DNDEBUG", "-ffunction-sections", "-fdata-sections"],
        opt_link_flags = ["-Wl,--gc-sections"],
        supports_start_end_lib = False,
        target_libc = "ucrt",
        target_system_name = "x86_64-w64-mingw32",
        tool_paths = _tool_paths(nix["windows"]),
        toolchain_identifier = "llvm-mingw",
        unfiltered_compile_flags = _REPRODUCIBLE,
    )
    _cc_toolchain("windows", _WINDOWS)

def nix_toolchains(nix):
    """Declares and names every toolchain of the `@nix` repository.

    Args:
        nix: the decoded toolchain description written by `nix/bazel.nix`.
    """
    native.filegroup(name = "empty")
    rust_stdlib_filegroup(name = "rust_std", srcs = [])

    _rust(
        name = "gnu",
        triple = "x86_64-unknown-linux-gnu",
        target_compatible_with = _LINUX + [_LIBC["gnu"]],
        env = _rust_environment(nix),
        extra_rustc_flags = _LINKING,
        stdlib_linkflags = ["-lpthread", "-ldl"],
    )
    _rust(
        name = "musl",
        triple = nix["musl"]["target"],
        target_compatible_with = _LINUX + [_LIBC["musl"]],
        env = _rust_environment(nix),
        extra_rustc_flags = [],
        stdlib_linkflags = ["-lpthread", "-ldl"],
    )
    _rust(
        name = "windows",
        triple = nix["windows"]["target"],
        target_compatible_with = _WINDOWS,
        env = _windows_environment(nix),
        extra_rustc_flags = ["-Ctarget-feature=+crt-static"],
        stdlib_linkflags = _WINDOWS_STDLIB,
        binary_ext = ".exe",
        dylib_ext = ".dll",
    )
    _cc(nix, "gnu")
    _cc(nix, "musl")
    _cc_windows(nix)

    cargo_toolchain(
        name = "cargo_toolchain",
        cargo = "rust/bin/cargo",
        channel = "nightly",
    )
    native.toolchain(
        name = "cargo",
        exec_compatible_with = _LINUX,
        toolchain = ":cargo_toolchain",
        toolchain_type = "@rules_rust//cargo:toolchain_type",
    )

    rustfmt_toolchain(
        name = "rustfmt_toolchain",
        rustc = "rust/bin/rustc",
        rustc_lib = ":empty",
        rustfmt = "rust/bin/rustfmt",
    )
    native.toolchain(
        name = "rustfmt",
        exec_compatible_with = _LINUX,
        toolchain = ":rustfmt_toolchain",
        toolchain_type = "@rules_rust//rust/rustfmt:toolchain_type",
    )

    native.filegroup(name = "rust_src", srcs = ["rust/src"])
    rust_analyzer_toolchain(
        name = "rust_analyzer_toolchain",
        proc_macro_srv = "rust/bin/rust-analyzer-proc-macro-srv",
        rust_analyzer = "rust/bin/rust-analyzer",
        rustc = "rust/bin/rustc",
        rustc_srcs = ":rust_src",
        rustc_srcs_path = "rust/src/library",
    )
    native.toolchain(
        name = "rust_analyzer",
        exec_compatible_with = _LINUX,
        target_compatible_with = _LINUX,
        toolchain = ":rust_analyzer_toolchain",
        toolchain_type = "@rules_rust//rust/rust_analyzer:toolchain_type",
    )

    sh_toolchain(name = "sh_toolchain", path = nix["bash"])
    for toolchain_type in ["@rules_shell//shell:toolchain_type", "@bazel_tools//tools/sh:toolchain_type"]:
        native.toolchain(
            name = "sh_" + toolchain_type.split("//")[0].lstrip("@"),
            exec_compatible_with = _LINUX,
            toolchain = ":sh_toolchain",
            toolchain_type = toolchain_type,
        )
