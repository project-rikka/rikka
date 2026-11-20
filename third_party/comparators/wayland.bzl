"""Generates the C glue of one Wayland protocol with the shell's wayland-scanner."""

load("@nix//:paths.bzl", "NIX")

def _wayland_protocol_impl(ctx):
    for mode, output in [("client-header", ctx.outputs.header), ("private-code", ctx.outputs.code)]:
        ctx.actions.run(
            executable = NIX["path"] + "/wayland-scanner",
            arguments = [mode, ctx.file.src.path, output.path],
            inputs = [ctx.file.src],
            outputs = [output],
            mnemonic = "WaylandScanner",
            progress_message = "Generating %{output}",
        )
    return [DefaultInfo(files = depset([ctx.outputs.header, ctx.outputs.code]))]

wayland_protocol = rule(
    implementation = _wayland_protocol_impl,
    doc = "Runs wayland-scanner over one protocol XML into a client header and its private code.",
    attrs = {
        "code": attr.output(mandatory = True, doc = "The generated private code."),
        "header": attr.output(mandatory = True, doc = "The generated client header."),
        "src": attr.label(allow_single_file = [".xml"], mandatory = True, doc = "The protocol XML."),
    },
)
