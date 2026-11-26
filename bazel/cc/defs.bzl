"""Rikka's C and C++ rules: programs over the SDK linked with the session's RUNPATH."""

load("@rules_cc//cc:cc_binary.bzl", "cc_binary")
load("//bazel/config:linkage.bzl", "linkage")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "hermetic_test")

_LINUX = ["@platforms//os:linux"]

def rikka_cc_binary(name, **kwargs):
    """A C or C++ executable for Linux linked with the session's RUNPATH.

    Args:
        name: target name and the built executable's file name.
        **kwargs: passed to `cc_binary`.
    """
    env = SHIPPED_ENV | kwargs.pop("env", {})
    cc_binary(
        name = name,
        env = env,
        linkopts = kwargs.pop("linkopts", []) + linkage(executable = True),
        target_compatible_with = _LINUX,
        **kwargs
    )

def rikka_cc_test(name, timeout = "short", **kwargs):
    """A C or C++ test program, run hermetically as one process within Bazel's short timeout.

    `<name>` is the hermetic launcher of the `<name>_bin` program.

    Args:
        name: target name.
        timeout: Bazel's timeout for the program.
        **kwargs: passed to `cc_binary`.
    """
    tags = kwargs.pop("tags", [])
    cc_binary(
        name = name + "_bin",
        linkopts = kwargs.pop("linkopts", []) + linkage(executable = True),
        tags = tags,
        target_compatible_with = _LINUX,
        testonly = True,
        **kwargs
    )
    hermetic_test(
        name = name,
        binary = ":" + name + "_bin",
        tags = tags,
        timeout = timeout,
    )
