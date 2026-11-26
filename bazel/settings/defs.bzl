"""Parts of the settings catalog `//:settings_catalog`, each printed by the crate that owns it."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("//bazel/rust:defs.bzl", "rikka_binary")

_SETTINGS = "//crates/session/rikka-settings"

def rikka_settings_catalog(name, catalog, deps = []):
    """A program that prints the settings domains its crate owns as part of the catalog.

    Args:
        name: target name of the program, listed in `//:settings_catalog`.
        catalog: the Rust path of a function returning a `rikka_settings::Schema` or
            `rikka_settings::Catalog`, such as `rikka_terminal::Config::schema`.
        deps: the crate that defines the function, unless it is `rikka-settings` itself.
    """
    main = name + "_main"
    write_file(
        name = main,
        out = name + "_main.rs",
        content = [
            "fn main() -> std::process::ExitCode {",
            "    rikka_settings::catalog::print({}())".format(catalog),
            "}",
            "",
        ],
    )
    rikka_binary(
        name = name,
        srcs = [":" + main],
        crate_root = ":" + main,
        deps = deps + [_SETTINGS],
        target_compatible_with = ["@platforms//os:linux"],
        visibility = ["//:__pkg__"],
    )
