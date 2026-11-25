"""Where development runs find what an installed Rikka keeps beside its programs."""

load("@nix//:paths.bzl", "NIX")

SHIPPED_ENV = {
    "RIKKA_GECKO_PATH": NIX["gecko"],
    "RIKKA_GRAMMAR_PATH": NIX["grammars"],
    "RIKKA_ICON_FONT_PATH": NIX["icon_fonts"],
    "RIKKA_MESA_PATH": NIX["mesa"],
    "RIKKA_MINECRAFT_PATH": NIX["minecraft_runtime"],
}
