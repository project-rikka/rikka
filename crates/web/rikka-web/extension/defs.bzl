"""Packs the Rikka Chrome extension with the scripts its users bundle, and tests that run it."""

load("@bazel_skylib//rules:run_binary.bzl", "run_binary")
load("@nix//:paths.bzl", "NIX")
load("//bazel/rust:defs.bzl", "rikka_test")

CORE = [
    "background.js",
    "claim.html",
    "claim.js",
    "context.js",
    "manifest.json",
    "page.js",
    "relay.js",
]

_TEST_CHROME = "//crates/web/rikka-test-chrome"

_WEB_HOST = "//system/rikka-web-host"

def rikka_web_extension(name, scripts = {}, **kwargs):
    """An unsigned extension zip: the core files plus `scripts` (path under `scripts/` to label).

    Args:
        name: target name; the output is `<name>.zip`.
        scripts: bundled scripts, keyed by their path under `scripts/`.
        **kwargs: passed to `run_binary`.
    """
    files = {
        path: Label("//crates/web/rikka-web/extension:" + path)
        for path in CORE
    } | {
        "scripts/" + path: label
        for path, label in scripts.items()
    }
    out = name + ".zip"
    run_binary(
        name = name,
        srcs = files.values(),
        outs = [out],
        args = ["$(execpath {})".format(out)] + [
            "{}=$(execpath {})".format(path, label)
            for path, label in files.items()
        ],
        tool = Label("//crates/web/rikka-web-pack"),
        **kwargs
    )

def rikka_web_machine_test(name, extension, **kwargs):
    """A machine test whose runtimes start the dev shell's Google Chrome with `extension` loaded.

    Chrome runs through `rikka-test-chrome`, which loads the unpacked extension under the Rikka
    key and hides the machine's Chrome policies, and reaches the test through `rikka-web-host`.

    Args:
        name: target name.
        extension: a `rikka_web_extension` holding the scripts the tests run.
        **kwargs: passed to `rikka_test`.
    """
    rikka_test(
        name = name,
        machine = True,
        data = kwargs.pop("data", []) + [extension, _TEST_CHROME, _WEB_HOST],
        env = kwargs.pop("env", {}) | {
            "RIKKA_CHROME_PATH": "$(rootpath {})".format(_TEST_CHROME),
            "RIKKA_TEST_CHROME": NIX["chrome"],
            "RIKKA_TEST_EXTENSION": "$(rootpath {})".format(extension),
            "RIKKA_WEB_HOST_PATH": "$(rootpath {})".format(_WEB_HOST),
        },
        **kwargs
    )
