"""Generates one side of every vendored Wayland protocol with the in-house scanner."""

def _rikka_wayland_protocols_impl(ctx):
    arguments = ctx.actions.args()
    arguments.add(ctx.attr.side)
    arguments.add(ctx.outputs.out)
    arguments.add_all(ctx.files._xml)
    ctx.actions.run(
        executable = ctx.executable._generate,
        arguments = [arguments],
        inputs = ctx.files._xml,
        outputs = [ctx.outputs.out],
        mnemonic = "WaylandProtocols",
        progress_message = "Generating %{output}",
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

rikka_wayland_protocols = rule(
    implementation = _rikka_wayland_protocols_impl,
    doc = "Runs the scanner over the vendored XML into one Rust file a crate root includes.",
    attrs = {
        "side": attr.string(mandatory = True, values = ["shared", "server", "client"]),
        "out": attr.output(mandatory = True),
        "_xml": attr.label(
            default = Label("//crates/foundation/rikka-wayland-protocols:xml"),
            allow_files = True,
        ),
        "_generate": attr.label(
            default = Label("//crates/foundation/rikka-wayland-protocols:generate"),
            executable = True,
            cfg = "exec",
        ),
    },
)
