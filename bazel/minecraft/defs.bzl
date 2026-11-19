"""Rikka's Fabric rules: mods compiled against the pinned Minecraft client, the client run with them, and its sources."""

load("@minecraft//:versions.bzl", "FABRIC_LOADER", "MINECRAFT")
load("@nix//:paths.bzl", "NIX")
load("//bazel/java:defs.bzl", "RikkaJavaInfo")

FabricModInfo = provider(
    doc = "A Fabric mod jar.",
    fields = {
        "jar": "the mod's jar",
        "libraries": "depset of shared libraries the mod loads",
    },
)

_MOD_METADATA = "fabric.mod.json"

_MAPPING_NAMESPACE = "official"

_MIXIN_REMAP_TYPE = "static"

_JAR_DATE = "1980-01-01T00:00:02Z"

_NATIVES_PLACEHOLDER = "${natives_directory}"

_LAUNCHER_PLACEHOLDERS = {
    "${launcher_name}": "rikka",
    "${launcher_version}": "dev",
}

_RUN_DIRECTORY = "run"

_SOURCES_DIRECTORY = "mcsrc"

_JAVAC_FLAGS = "-encoding UTF-8 -g -proc:none -implicit:none -Xlint:all,-classfile -Werror"

_COMPILE = """set -eu
export PATH="{path}"
classes="$(mktemp -d)"
trap 'rm -rf "$classes"' EXIT
"{java_home}/bin/javac" --release {release} {flags} -d "$classes" -classpath "{classpath}" {sources}
bundled=({bundled})
for library in "${{bundled[@]}}"; do
    (cd "$classes" && "{java_home}/bin/jar" --extract --file "$OLDPWD/$library")
done
rm -rf "$classes/module-info.class" "$classes/META-INF"
while [ "$#" -gt 0 ]; do
    mkdir -p "$classes/$(dirname "$2")"
    cp "$1" "$classes/$2"
    shift 2
done
"{java_home}/bin/jar" --create --file "{output}" --manifest "{manifest}" --date={date} -C "$classes" .
"""

_MANIFEST = """Manifest-Version: 1.0
Fabric-Mapping-Namespace: {namespace}
Fabric-Minecraft-Version: {minecraft}
Fabric-Loader-Version: {loader}
Fabric-Mixin-Group: {mixin_group}
Fabric-Mixin-Version: {mixin_version}
"""

def _game_attrs():
    return {
        "_client": attr.label(default = "@minecraft//:client", allow_files = True),
        "_libraries": attr.label(default = "@minecraft//:libraries", allow_files = True),
        "_fabric_loader": attr.label(default = "@minecraft//:fabric_loader", allow_files = True),
        "_fabric_loader_development": attr.label(
            default = "@minecraft//:fabric_loader_development",
            allow_files = True,
        ),
    }

def _game_jars(ctx):
    return ctx.files._libraries + ctx.files._fabric_loader + ctx.files._fabric_loader_development + ctx.files._client

def _resource_path(ctx, resource):
    prefix = "/".join([part for part in [ctx.label.package, ctx.attr.resource_strip_prefix] if part]) + "/"
    if not resource.short_path.startswith(prefix):
        fail("{} is outside {}".format(resource.short_path, prefix))
    return resource.short_path[len(prefix):]

def _fabric_mod_impl(ctx):
    resources = []
    for resource in ctx.files.resources:
        path = _resource_path(ctx, resource)
        if path == _MOD_METADATA:
            expanded = ctx.actions.declare_file(ctx.label.name + "/" + _MOD_METADATA)
            ctx.actions.expand_template(
                template = resource,
                output = expanded,
                substitutions = {
                    "${version}": ctx.attr.version,
                    "${minecraft_version}": MINECRAFT.version,
                    "${loader_version}": FABRIC_LOADER.version,
                    "${java_release}": str(MINECRAFT.java_release),
                },
            )
            resource = expanded
        resources.append(struct(file = resource, path = path))
    if _MOD_METADATA not in [resource.path for resource in resources]:
        fail("a Fabric mod needs a {} at the root of its resources".format(_MOD_METADATA))

    manifest = ctx.actions.declare_file(ctx.label.name + ".MF")
    ctx.actions.write(manifest, _MANIFEST.format(
        namespace = _MAPPING_NAMESPACE,
        minecraft = MINECRAFT.version,
        loader = FABRIC_LOADER.version,
        mixin_group = FABRIC_LOADER.mixin_group,
        mixin_version = FABRIC_LOADER.mixin_version,
    ))

    jar = ctx.actions.declare_file(ctx.label.name + ".jar")
    bundled = depset(transitive = [dep[RikkaJavaInfo].jars for dep in ctx.attr.deps]).to_list()
    libraries = depset(transitive = [dep[RikkaJavaInfo].libraries for dep in ctx.attr.deps])
    classpath = _game_jars(ctx) + bundled
    pairs = []
    for resource in resources:
        pairs.extend([resource.file.path, resource.path])
    ctx.actions.run(
        executable = NIX["bash"],
        outputs = [jar],
        inputs = ctx.files.srcs + classpath + [manifest] + [resource.file for resource in resources],
        arguments = ["-c", _COMPILE.format(
            path = NIX["path"],
            java_home = NIX["java_home"],
            release = MINECRAFT.java_release,
            flags = _JAVAC_FLAGS,
            classpath = ":".join([file.path for file in classpath]),
            bundled = " ".join([library.path for library in bundled]),
            sources = " ".join([source.path for source in ctx.files.srcs]),
            output = jar.path,
            manifest = manifest.path,
            date = _JAR_DATE,
        ), "fabric_mod"] + pairs,
        mnemonic = "FabricMod",
        progress_message = "Building Fabric mod %{label}",
        use_default_shell_env = False,
    )
    return [DefaultInfo(files = depset([jar])), FabricModInfo(jar = jar, libraries = libraries)]

fabric_mod = rule(
    implementation = _fabric_mod_impl,
    doc = "A Fabric mod jar compiled against the pinned Minecraft client and Fabric Loader.",
    attrs = _game_attrs() | {
        "srcs": attr.label_list(allow_files = [".java"], mandatory = True),
        "deps": attr.label_list(
            providers = [RikkaJavaInfo],
            doc = "Java libraries compiled against and bundled into the mod jar.",
        ),
        "resources": attr.label_list(allow_files = True, mandatory = True),
        "resource_strip_prefix": attr.string(
            default = "src/main/resources",
            doc = "The package-relative folder whose layout the jar keeps.",
        ),
        "version": attr.string(mandatory = True, doc = "Expands `${version}` in fabric.mod.json."),
    },
)

def _quote(text):
    return "'" + text.replace("'", "'\\''") + "'"

def _shell_word(argument):
    for placeholder, value in _LAUNCHER_PLACEHOLDERS.items():
        argument = argument.replace(placeholder, value)
    parts = argument.split(_NATIVES_PLACEHOLDER)
    return "\"$natives\"".join([_quote(part) for part in parts])

def _runfile(file):
    return "$runfiles/" + file.short_path

def _package_path(ctx, name):
    return "/".join([part for part in [ctx.label.package, name] if part])

def _minecraft_client_impl(ctx):
    mods = [mod[FabricModInfo].jar for mod in ctx.attr.mods]
    libraries = depset(transitive = [mod[FabricModInfo].libraries for mod in ctx.attr.mods]).to_list()
    library_dirs = sorted({"$runfiles/" + library.short_path.rpartition("/")[0]: None for library in libraries}.keys())
    classpath = _game_jars(ctx) + mods
    index = ctx.file._asset_index
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.expand_template(
        template = ctx.file._launcher,
        output = launcher,
        substitutions = {
            "{bash}": NIX["bash"],
            "{label}": str(ctx.label),
            "{run_directory}": _package_path(ctx, _RUN_DIRECTORY),
            "{assets}": index.short_path.rpartition("/indexes/")[0],
            "{library_path}": ":".join(library_dirs + [NIX["runpath"], NIX["minecraft_library_path"]]),
            "{java}": NIX["java_home"] + "/bin/java",
            "{jvm_arguments}": " ".join([_shell_word(argument) for argument in MINECRAFT.jvm_arguments]),
            "{namespace}": _MAPPING_NAMESPACE,
            "{remap_type}": _MIXIN_REMAP_TYPE,
            "{log4j}": _runfile(ctx.file._log4j),
            "{classpath}": ":".join([_runfile(file) for file in classpath]),
            "{main_class}": FABRIC_LOADER.main_class,
            "{asset_index}": MINECRAFT.asset_index,
        },
        is_executable = True,
    )
    runfiles = ctx.runfiles(files = classpath + libraries + [index, ctx.file._log4j] + ctx.files._asset_objects)
    return [DefaultInfo(executable = launcher, runfiles = runfiles)]

minecraft_client = rule(
    implementation = _minecraft_client_impl,
    executable = True,
    doc = "Starts the pinned Minecraft client under Fabric Loader with the given mods, in `<package>/run`.",
    attrs = _game_attrs() | {
        "mods": attr.label_list(providers = [FabricModInfo]),
        "_asset_index": attr.label(default = "@minecraft_assets//:index", allow_single_file = True),
        "_asset_objects": attr.label(default = "@minecraft_assets//:objects", allow_files = True),
        "_log4j": attr.label(default = Label("//bazel/minecraft:log4j2.xml"), allow_single_file = True),
        "_launcher": attr.label(default = Label("//bazel/minecraft:client.sh.tpl"), allow_single_file = True),
    },
)

def _minecraft_sources_impl(ctx):
    sources = ctx.actions.declare_file("{}/minecraft-{}-sources.jar".format(ctx.label.name, MINECRAFT.version))
    client = ctx.file._client
    arguments = ctx.actions.args()
    arguments.add("-jar", ctx.file._vineflower)
    arguments.add_all([
        "--log-level=warn",
        "--indent-string=\t",
        "--bytecode-source-mapping",
        "--file",
    ])
    arguments.add_all(ctx.files._libraries, format_each = "--add-external=%s")
    arguments.add_all([client, sources])
    ctx.actions.run(
        executable = NIX["java_home"] + "/bin/java",
        outputs = [sources],
        inputs = [ctx.file._vineflower, client] + ctx.files._libraries,
        arguments = [arguments],
        mnemonic = "Decompile",
        progress_message = "Decompiling Minecraft {}".format(MINECRAFT.version),
        use_default_shell_env = False,
    )
    unpacker = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.expand_template(
        template = ctx.file._unpacker,
        output = unpacker,
        substitutions = {
            "{bash}": NIX["bash"],
            "{label}": str(ctx.label),
            "{jar}": NIX["java_home"] + "/bin/jar",
            "{sources}": sources.short_path,
            "{destination}": _package_path(ctx, _SOURCES_DIRECTORY),
            "{version}": MINECRAFT.version,
        },
        is_executable = True,
    )
    return [DefaultInfo(
        executable = unpacker,
        files = depset([sources]),
        runfiles = ctx.runfiles(files = [sources]),
    )]

minecraft_sources = rule(
    implementation = _minecraft_sources_impl,
    executable = True,
    doc = "The pinned Minecraft client decompiled by Vineflower; running it unpacks the sources into `<package>/mcsrc`.",
    attrs = {
        "_client": attr.label(default = "@minecraft//:client", allow_single_file = True),
        "_libraries": attr.label(default = "@minecraft//:libraries", allow_files = True),
        "_vineflower": attr.label(default = "@minecraft//:vineflower", allow_single_file = True),
        "_unpacker": attr.label(default = Label("//bazel/minecraft:sources.sh.tpl"), allow_single_file = True),
    },
)
