"""The hermetic launcher every Rikka test runs its program through."""

load("@nix//:paths.bzl", "NIX")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", "rustfmt_aspect")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")

LIBTEST = "libtest"

PROGRAM = "program"

_LINT_GROUPS = ["clippy_checks", "clippy_output", "rustfmt_checks"]

def _hermetic_test_impl(ctx):
    binary = ctx.attr.binary[DefaultInfo]
    executable = binary.files_to_run.executable
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.expand_template(
        template = ctx.file._launcher,
        output = launcher,
        substitutions = {
            "{bash}": NIX["bash"],
            "{binary}": executable.short_path,
            "{fontconfig_file}": NIX["fontconfig_file"],
            "{jobs}": str(ctx.attr.jobs),
            "{limit}": str(ctx.attr.limit),
            "{path}": NIX["path"],
            "{runner}": ctx.attr.runner,
            "{ssl_cert_file}": NIX["ssl_cert_file"],
            "{xdg_data_dirs}": NIX["xdg_data_dirs"],
        },
        is_executable = True,
    )
    inherited = ctx.attr.binary[RunEnvironmentInfo] if RunEnvironmentInfo in ctx.attr.binary else RunEnvironmentInfo()
    environment = RunEnvironmentInfo(
        environment = SHIPPED_ENV | inherited.environment,
        inherited_environment = inherited.inherited_environment,
    )
    groups = ctx.attr.binary[OutputGroupInfo] if OutputGroupInfo in ctx.attr.binary else None
    lints = {group: getattr(groups, group) for group in _LINT_GROUPS if groups and hasattr(groups, group)}
    return [
        DefaultInfo(
            executable = launcher,
            runfiles = ctx.runfiles(files = [executable]).merge(binary.default_runfiles),
        ),
        OutputGroupInfo(**lints),
        environment,
    ]

_hermetic_test = rule(
    implementation = _hermetic_test_impl,
    test = True,
    doc = """Runs `binary` from the runfiles root with a scratch home, XDG and runtime tree and no session sockets.

    The `libtest` runner lists the binary's tests and runs each in its own process and scratch tree with
    `limit` seconds, up to `jobs` at once, spread over Bazel's shards; the `program` runner runs the binary
    once. Test arguments reach the binary, and its environment is the binary's own. A Rust binary's
    clippy and rustfmt checks are the launcher's own output groups, so `--config=check` lints it
    through the launcher.
    """,
    attrs = {
        "binary": attr.label(
            aspects = [rust_clippy_aspect, rustfmt_aspect],
            cfg = "target",
            executable = True,
            mandatory = True,
        ),
        "jobs": attr.int(default = 1, doc = "Tests of a shard that run at once."),
        "limit": attr.int(default = 60, doc = "Seconds each test may take."),
        "runner": attr.string(default = PROGRAM, values = [LIBTEST, PROGRAM]),
        "_launcher": attr.label(default = ":hermetic.sh.tpl", allow_single_file = True),
    },
)

def hermetic_test(name, **kwargs):
    """The hermetic launcher of a test program; tests run only on Linux.

    Args:
        name: target name.
        **kwargs: passed to the launcher rule.
    """
    _hermetic_test(
        name = name,
        target_compatible_with = ["@platforms//os:linux"] + kwargs.pop("target_compatible_with", []),
        **kwargs
    )
