"""Benchmark rules: every target is test-only, built on request and compiled by LLVM.

Comparators come from `@bench_crates` and Rikka's crates from `@crates`, so a program may link the
same crate version from both universes; their unmangled symbols are identical, and the linker keeps
one of each.
"""

load("@rules_cc//cc:cc_library.bzl", "cc_library")
load("//bazel/rust:defs.bzl", "rikka_binary", "rikka_library", "rikka_test")

_BENCHMARK = {
    "tags": ["manual"],
    "testonly": True,
}

_BOTH_UNIVERSES = ["-Clink-arg=-Wl,--allow-multiple-definition"]

def bench_library(name, **kwargs):
    """A Rust library of a benchmark suite.

    Args:
        name: target name.
        **kwargs: passed to `rikka_library`.
    """
    rikka_library(name = name, cranelift = False, **(kwargs | _BENCHMARK))

def bench_binary(name, visibility = ["//benchmarks:__pkg__"], **kwargs):
    """A benchmark program, run with `bazel run --config=bench` and listed in `//benchmarks:programs`.

    Args:
        name: target name.
        visibility: who may depend on the program.
        **kwargs: passed to `rikka_binary`.
    """
    rustc_flags = kwargs.pop("rustc_flags", []) + _BOTH_UNIVERSES
    rikka_binary(name = name, cranelift = False, rustc_flags = rustc_flags, visibility = visibility, **(kwargs | _BENCHMARK))

def bench_test(name, **kwargs):
    """A suite's parity test, run through `//benchmarks:parity`.

    Args:
        name: target name.
        **kwargs: passed to `rikka_test`.
    """
    rustc_flags = kwargs.pop("rustc_flags", []) + _BOTH_UNIVERSES
    rikka_test(name = name, rustc_flags = rustc_flags, **(kwargs | _BENCHMARK))

def bench_cc_library(name, **kwargs):
    """A C or C++ library binding a native comparator.

    Args:
        name: target name.
        **kwargs: passed to `cc_library`.
    """
    cc_library(name = name, **(kwargs | _BENCHMARK))
