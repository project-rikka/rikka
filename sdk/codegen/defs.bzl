"""SDK bindings generated from `//sdk:rikka.ridl` by `rikka-sdk-codegen`."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("@bazel_skylib//rules:run_binary.bzl", "run_binary")

_IDL = "//sdk:rikka.ridl"

_CODEGEN = "//sdk/codegen:rikka-sdk-codegen"

GENERATED = "generated/"

def rikka_bindings(name, language, outs, committed = True, visibility = None):
    """Generates one language's bindings into this package.

    Committed bindings are written to `generated/<out>` and checked against the copy committed at
    `<out>`, which `bazel run //xtask -- sdk` refreshes. Build-time bindings are written to `<out>`.

    Args:
        name: target name of the generated files.
        language: the codegen language, such as `c`, `cpp` or `rust`.
        outs: every file the language produces, relative to this package.
        committed: whether the files are committed to the source tree.
        visibility: visibility of the generated files.
    """
    generated = [GENERATED + out for out in outs] if committed else outs
    run_binary(
        name = name,
        srcs = [_IDL],
        outs = generated,
        args = ["$(execpath %s)" % _IDL, language] + ["$(execpath %s)" % out for out in generated],
        tool = _CODEGEN,
        visibility = visibility,
    )
    if not committed:
        return
    for out in outs:
        diff_test(
            name = "%s_%s_is_current" % (name, out.replace("/", "_").replace(".", "_")),
            file1 = GENERATED + out,
            file2 = out,
            failure_message = "%s/%s is stale; regenerate it with `bazel run //xtask -- sdk`" % (native.package_name(), out),
            target_compatible_with = ["@platforms//os:linux"],
        )
