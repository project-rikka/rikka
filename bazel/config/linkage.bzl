"""How linked programs find their libraries.

Development builds carry the dev shell's RUNPATH. Portable builds (`--config=dist`, the `linux_portable` platform) carry no
RUNPATH and the standard interpreter, so the distribution's package relinks them against nixpkgs.
"""

load("@nix//:paths.bzl", "NIX")

_INTERPRETER = "/lib64/ld-linux-x86-64.so.2"

def linkage(executable, prefix = ""):
    """Linker flags for a linked artifact.

    Args:
        executable: whether the artifact is a program, which names its interpreter.
        prefix: prepended to every flag, such as `-Clink-arg=` for rustc.

    Returns:
        A `select` of linker flags.
    """
    portable = ["-Wl,--dynamic-linker=" + _INTERPRETER] if executable else []
    return select({
        Label("//bazel/platforms:musl"): [],
        Label("//bazel/platforms:portable"): [prefix + flag for flag in portable],
        Label("@platforms//os:windows"): [],
        "//conditions:default": [prefix + "-Wl,-rpath," + NIX["runpath"]],
    })
