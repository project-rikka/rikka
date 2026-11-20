"""Rikka's C# rules: assemblies compiled by the development shell's .NET SDK and run by its runtime."""

load("@nix//:paths.bzl", "NIX")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "hermetic_test")

RikkaCSharpInfo = provider(
    doc = "A compiled assembly with the assemblies and shared libraries it needs at run time.",
    fields = {
        "assembly": "the compiled assembly",
        "assemblies": "depset of every assembly it references, itself included",
        "libraries": "depset of shared libraries it loads",
    },
)

_COMPILE = """set -eu
export PATH="{path}" HOME="$(mktemp -d)" DOTNET_ROOT="{dotnet}"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1 DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
trap 'rm -rf "$HOME"' EXIT
compiler="$(ls -d "$DOTNET_ROOT"/sdk/*/Roslyn/bincore/csc.dll | tail -n 1)"
pack="$(ls -d "$DOTNET_ROOT"/packs/Microsoft.NETCore.App.Ref/* | tail -n 1)"
references="$(ls -d "$pack"/ref/net* | tail -n 1)"
arguments="$HOME/arguments.rsp"
{{
    echo -nologo -nostdlib+ -deterministic+ -optimize+ -unsafe+ -debug:portable
    echo -nullable:enable -langversion:{language} -warnaserror+ -nowarn:CS1591 -target:{target}
    echo "-out:{output}"
    for reference in "$references"/*.dll; do echo "-reference:$reference"; done
    for analyzer in "$pack"/analyzers/dotnet/cs/*.dll; do echo "-analyzer:$analyzer"; done
    for reference in {references}; do echo "-reference:$reference"; done
    {documentation}
    for source in {sources}; do echo "$source"; done
}} > "$arguments"
"$DOTNET_ROOT/dotnet" exec "$compiler" -noconfig "@$arguments"
"""

def _compile(ctx, output, target, references, documentation = None):
    outputs = [output] + ([documentation] if documentation else [])
    ctx.actions.run(
        executable = NIX["bash"],
        outputs = outputs,
        inputs = ctx.files.srcs + references,
        arguments = ["-c", _COMPILE.format(
            path = NIX["path"],
            dotnet = NIX["dotnet"],
            language = ctx.attr.language,
            target = target,
            output = output.path,
            references = " ".join([reference.path for reference in references]),
            documentation = 'echo "-doc:{}"'.format(documentation.path) if documentation else "",
            sources = " ".join([source.path for source in ctx.files.srcs]),
        )],
        mnemonic = "CSharpCompile",
        progress_message = "Compiling C# %{label}",
        use_default_shell_env = False,
    )

def _transitive(deps):
    assemblies = depset(transitive = [dep[RikkaCSharpInfo].assemblies for dep in deps])
    libraries = depset(transitive = [dep[RikkaCSharpInfo].libraries for dep in deps])
    return assemblies, libraries

def _library_impl(ctx):
    assembly = ctx.actions.declare_file(ctx.attr.assembly + ".dll")
    documentation = ctx.actions.declare_file(ctx.attr.assembly + ".xml")
    assemblies, libraries = _transitive(ctx.attr.deps)
    _compile(ctx, assembly, "library", assemblies.to_list(), documentation)
    return [
        DefaultInfo(files = depset([assembly, documentation])),
        RikkaCSharpInfo(
            assembly = assembly,
            assemblies = depset([assembly], transitive = [assemblies]),
            libraries = depset(ctx.files.libraries, transitive = [libraries]),
        ),
    ]

_ATTRS = {
    "srcs": attr.label_list(allow_files = [".cs"], mandatory = True),
    "deps": attr.label_list(providers = [RikkaCSharpInfo]),
    "language": attr.string(default = "12", doc = "The C# language version."),
}

rikka_csharp_library = rule(
    implementation = _library_impl,
    doc = "A class library assembly with its XML documentation.",
    attrs = _ATTRS | {
        "assembly": attr.string(mandatory = True, doc = "The assembly name."),
        "libraries": attr.label_list(allow_files = True, doc = "Shared libraries loaded at run time."),
    },
)

_RUNTIME_CONFIG = """{
  "runtimeOptions": {
    "tfm": "net8.0",
    "rollForward": "LatestMajor",
    "framework": { "name": "Microsoft.NETCore.App", "version": "8.0.0" }
  }
}
"""

_LAUNCHER = """#!{bash}
set -eu
export DOTNET_ROOT="{dotnet}" DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
export LD_LIBRARY_PATH="{libraries}"
exec "$DOTNET_ROOT/dotnet" exec --runtimeconfig "$PWD/{config}" "$PWD/{program}" "$@"
"""

def _program_impl(ctx):
    name = ctx.label.name
    assemblies, libraries = _transitive(ctx.attr.deps)
    program = ctx.actions.declare_file("{0}/{0}.dll".format(name))
    _compile(ctx, program, "exe", assemblies.to_list())
    copies = []
    for assembly in assemblies.to_list():
        copy = ctx.actions.declare_file("{}/{}".format(name, assembly.basename))
        ctx.actions.symlink(output = copy, target_file = assembly)
        copies.append(copy)
    config = ctx.actions.declare_file("{0}/{0}.runtimeconfig.json".format(name))
    ctx.actions.write(config, _RUNTIME_CONFIG)
    launcher = ctx.actions.declare_file(name + ".sh")
    library_dirs = sorted({"$PWD/" + library.short_path.rpartition("/")[0]: None for library in libraries.to_list()}.keys())
    ctx.actions.write(
        launcher,
        _LAUNCHER.format(
            bash = NIX["bash"],
            dotnet = NIX["dotnet"],
            libraries = ":".join(library_dirs),
            config = config.short_path,
            program = program.short_path,
        ),
        is_executable = True,
    )
    runfiles = ctx.runfiles(files = [program, config] + copies, transitive_files = libraries)
    return [
        DefaultInfo(executable = launcher, runfiles = runfiles),
        RunEnvironmentInfo(environment = SHIPPED_ENV | ctx.attr.env),
    ]

_PROGRAM_ATTRS = _ATTRS | {
    "env": attr.string_dict(),
}

rikka_csharp_binary = rule(
    implementation = _program_impl,
    executable = True,
    doc = "A C# program compiled to an executable assembly and run by the shell's .NET runtime from the runfiles root.",
    attrs = _PROGRAM_ATTRS,
)

def rikka_csharp_test(name, srcs, deps = [], env = {}, tags = [], timeout = "short"):
    """A C# test program, compiled to an executable assembly and run hermetically as one process.

    `<name>` is the hermetic launcher of the `<name>_bin` program.

    Args:
        name: target name.
        srcs: the program's sources; its entry point exits non-zero on failure.
        deps: `rikka_csharp_library` assemblies it references.
        env: environment of the program.
        tags: tags of both targets.
        timeout: Bazel's timeout for the program.
    """
    rikka_csharp_binary(
        name = name + "_bin",
        srcs = srcs,
        deps = deps,
        env = env,
        tags = tags,
        testonly = True,
    )
    hermetic_test(
        name = name,
        binary = ":" + name + "_bin",
        tags = tags,
        timeout = timeout,
    )
