"""Fetches what `minecraft/minecraft.lock.json` pins: jars as `@minecraft`, the client's assets as `@minecraft_assets`."""

_ASSET_OBJECTS = "https://resources.download.minecraft.net/"

_BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

_HEX = "0123456789abcdef"

_MIXIN = "sponge-mixin"

_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(name = "client", srcs = {client})

filegroup(name = "libraries", srcs = {libraries})

filegroup(name = "fabric_loader", srcs = {fabric_loader})

filegroup(name = "fabric_loader_development", srcs = {fabric_loader_development})

filegroup(name = "vineflower", srcs = {vineflower})
"""

_VERSIONS = """MINECRAFT = struct(
    version = {minecraft_version},
    java_release = {java_release},
    asset_index = {asset_index},
    jvm_arguments = {jvm_arguments},
)

FABRIC_LOADER = struct(
    version = {loader_version},
    main_class = {main_class},
    mixin_group = {mixin_group},
    mixin_version = {mixin_version},
)
"""

_ASSETS_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(name = "index", srcs = [{index}])

filegroup(name = "objects", srcs = glob(["objects/**"]))
"""

def _download(rctx, groups):
    pending = [
        rctx.download(artifact["url"], artifact["path"], sha256 = artifact["sha256"], block = False)
        for artifacts in groups.values()
        for artifact in artifacts
    ]
    for download in pending:
        download.wait()
    return {name: repr([artifact["path"] for artifact in artifacts]) for name, artifacts in groups.items()}

def _mixin(libraries):
    for library in libraries:
        group, artifact, version = library["name"].split(":")[:3]
        if artifact == _MIXIN:
            return group, version
    fail("Fabric Loader pins no {}".format(_MIXIN))

def _minecraft_repository_impl(rctx):
    lock = json.decode(rctx.read(rctx.attr.lock))
    minecraft = lock["minecraft"]
    loader = lock["fabric_loader"]
    mixin_group, mixin_version = _mixin(loader["libraries"])

    rctx.file("BUILD.bazel", _BUILD.format(**_download(rctx, {
        "client": [minecraft["client"]],
        "libraries": minecraft["libraries"],
        "fabric_loader": [loader["jar"]] + loader["libraries"],
        "fabric_loader_development": loader["development_libraries"],
        "vineflower": [lock["vineflower"]["jar"]],
    })))
    rctx.file("versions.bzl", _VERSIONS.format(
        minecraft_version = repr(minecraft["version"]),
        java_release = repr(minecraft["java_release"]),
        asset_index = repr(minecraft["asset_index"]["id"]),
        jvm_arguments = repr(minecraft["jvm_arguments"]),
        loader_version = repr(loader["version"]),
        main_class = repr(loader["main_class"]),
        mixin_group = repr(mixin_group),
        mixin_version = repr(mixin_version),
    ))

minecraft_repository = repository_rule(
    implementation = _minecraft_repository_impl,
    attrs = {"lock": attr.label(mandatory = True, allow_single_file = True)},
)

def _sha1_integrity(digest):
    data = [_HEX.index(digest[i]) * 16 + _HEX.index(digest[i + 1]) for i in range(0, len(digest), 2)]
    encoded = []
    for start in range(0, len(data), 3):
        chunk = data[start:start + 3]
        padded = chunk + [0] * (3 - len(chunk))
        value = padded[0] << 16 | padded[1] << 8 | padded[2]
        sextets = [_BASE64[(value >> shift) & 63] for shift in (18, 12, 6, 0)]
        encoded.append("".join(sextets[:len(chunk) + 1]) + "=" * (3 - len(chunk)))
    return "sha1-" + "".join(encoded)

def _minecraft_assets_impl(rctx):
    index = "indexes/{}.json".format(rctx.attr.id)
    rctx.download(rctx.attr.url, index, sha256 = rctx.attr.sha256)
    digests = {entry["hash"]: None for entry in json.decode(rctx.read(index))["objects"].values()}
    pending = [
        rctx.download(
            "{}{}/{}".format(_ASSET_OBJECTS, digest[:2], digest),
            "objects/{}/{}".format(digest[:2], digest),
            integrity = _sha1_integrity(digest),
            block = False,
        )
        for digest in digests
    ]
    for download in pending:
        download.wait()
    rctx.file("BUILD.bazel", _ASSETS_BUILD.format(index = repr(index)))

minecraft_assets = repository_rule(
    implementation = _minecraft_assets_impl,
    attrs = {
        "id": attr.string(mandatory = True),
        "url": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
    },
)

def _minecraft_impl(mctx):
    locks = [tag.file for module in mctx.modules for tag in module.tags.lock]
    if len(locks) != 1:
        fail("expected one minecraft.lock tag, found {}".format(len(locks)))
    index = json.decode(mctx.read(locks[0]))["minecraft"]["asset_index"]
    minecraft_repository(name = "minecraft", lock = locks[0])
    minecraft_assets(name = "minecraft_assets", id = index["id"], url = index["url"], sha256 = index["sha256"])
    return mctx.extension_metadata(
        root_module_direct_deps = ["minecraft", "minecraft_assets"],
        root_module_direct_dev_deps = [],
        reproducible = True,
    )

minecraft = module_extension(
    implementation = _minecraft_impl,
    tag_classes = {"lock": tag_class(attrs = {"file": attr.label(mandatory = True)})},
)
