"""Rikka's Java rules: jars compiled by the development shell's JDK and run by its JVM."""

load("@nix//:paths.bzl", "NIX")
load("//bazel/shipped:defs.bzl", "SHIPPED_ENV")
load("//bazel/test:defs.bzl", "hermetic_test")

RikkaJavaInfo = provider(
    doc = "A compiled jar with the jars and shared libraries it needs at run time.",
    fields = {
        "jars": "depset of the jar and every jar it depends on",
        "libraries": "depset of shared libraries it loads",
    },
)

_COMPILE = """set -eu
export PATH="{path}" JAVA_HOME="{java_home}"
classes="$(mktemp -d)"
trap 'rm -rf "$classes"' EXIT
"$JAVA_HOME/bin/javac" --release {release} -encoding UTF-8 -Xlint:all -Xdoclint:all,-missing -Werror \\
    -implicit:none -d "$classes" {classpath} {sources}
"$JAVA_HOME/bin/jar" --create --file "{output}" --date=1980-01-01T00:00:02Z -C "$classes" .
"""

def _compile(ctx, output, jars):
    ctx.actions.run(
        executable = NIX["bash"],
        outputs = [output],
        inputs = ctx.files.srcs + jars,
        arguments = ["-c", _COMPILE.format(
            path = NIX["path"],
            java_home = NIX["java_home"],
            release = ctx.attr.release,
            classpath = "-classpath " + ":".join([jar.path for jar in jars]) if jars else "",
            sources = " ".join([source.path for source in ctx.files.srcs]),
            output = output.path,
        )],
        mnemonic = "JavaCompile",
        progress_message = "Compiling Java %{label}",
        use_default_shell_env = False,
    )

def _transitive(deps):
    jars = depset(transitive = [dep[RikkaJavaInfo].jars for dep in deps])
    libraries = depset(transitive = [dep[RikkaJavaInfo].libraries for dep in deps])
    return jars, libraries

def _library_impl(ctx):
    jar = ctx.actions.declare_file(ctx.label.name + ".jar")
    jars, libraries = _transitive(ctx.attr.deps)
    _compile(ctx, jar, jars.to_list())
    return [
        DefaultInfo(files = depset([jar])),
        RikkaJavaInfo(
            jars = depset([jar], transitive = [jars]),
            libraries = depset(ctx.files.libraries, transitive = [libraries]),
        ),
    ]

_ATTRS = {
    "srcs": attr.label_list(allow_files = [".java"], mandatory = True),
    "deps": attr.label_list(providers = [RikkaJavaInfo]),
    "release": attr.string(default = "22", doc = "The Java SE release compiled against."),
}

rikka_java_library = rule(
    implementation = _library_impl,
    doc = "A jar of Java sources, modular when they include a module-info.java.",
    attrs = _ATTRS | {
        "libraries": attr.label_list(allow_files = True, doc = "Shared libraries loaded at run time."),
    },
)

_LAUNCHER = """#!{bash}
set -eu
export LD_LIBRARY_PATH="{libraries}"
exec "{java_home}/bin/java" --enable-native-access=ALL-UNNAMED -Xshare:auto -classpath "{classpath}" {main} "$@"
"""

def _program_impl(ctx):
    jars, libraries = _transitive(ctx.attr.deps)
    jar = ctx.actions.declare_file(ctx.label.name + ".jar")
    _compile(ctx, jar, jars.to_list())
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    library_dirs = sorted({"$PWD/" + library.short_path.rpartition("/")[0]: None for library in libraries.to_list()}.keys())
    ctx.actions.write(
        launcher,
        _LAUNCHER.format(
            bash = NIX["bash"],
            java_home = NIX["java_home"],
            libraries = ":".join(library_dirs),
            classpath = ":".join(["$PWD/" + file.short_path for file in [jar] + jars.to_list()]),
            main = ctx.attr.main_class,
        ),
        is_executable = True,
    )
    runfiles = ctx.runfiles(files = [jar], transitive_files = depset(transitive = [jars, libraries]))
    return [
        DefaultInfo(executable = launcher, runfiles = runfiles),
        RunEnvironmentInfo(environment = SHIPPED_ENV | ctx.attr.env),
    ]

_PROGRAM_ATTRS = _ATTRS | {
    "main_class": attr.string(mandatory = True, doc = "The class whose main method runs."),
    "env": attr.string_dict(),
}

rikka_java_binary = rule(
    implementation = _program_impl,
    executable = True,
    doc = "A Java program run by the shell's JVM from the runfiles root.",
    attrs = _PROGRAM_ATTRS,
)

def rikka_java_test(name, srcs, main_class, deps = [], env = {}, tags = [], timeout = "short"):
    """A Java test program run hermetically as one process; its main method exits non-zero on failure.

    `<name>` is the hermetic launcher of the `<name>_bin` program.

    Args:
        name: target name.
        srcs: the program's sources.
        main_class: the class whose main method runs.
        deps: `rikka_java_library` jars it uses.
        env: environment of the program.
        tags: tags of both targets.
        timeout: Bazel's timeout for the program.
    """
    rikka_java_binary(
        name = name + "_bin",
        srcs = srcs,
        main_class = main_class,
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
