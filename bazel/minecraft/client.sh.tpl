#!{bash}
set -eu
if [ -z "${BUILD_WORKSPACE_DIRECTORY:-}" ]; then
    echo "{label}: start it with bazel run" >&2
    exit 1
fi
runfiles="$PWD"
game="$BUILD_WORKSPACE_DIRECTORY/{run_directory}"
assets="$game/assets"
natives="$game/natives"
mkdir -p "$assets" "$natives"
ln -sfn "$runfiles/{assets}/indexes" "$assets/indexes"
ln -sfn "$runfiles/{assets}/objects" "$assets/objects"
disable_ansi=true
if [ -t 1 ]; then
    disable_ansi=false
fi
cd "$game"
export LD_LIBRARY_PATH="{library_path}"
exec "{java}" {jvm_arguments} \
    -Dfabric.development=true \
    -Dfabric.defaultModDistributionNamespace={namespace} \
    -Dfabric.defaultMixinRemapType={remap_type} \
    -Dfabric.log.disableAnsi="$disable_ansi" \
    -Dlog4j2.formatMsgNoLookups=true \
    -Dlog4j.configurationFile="{log4j}" \
    -classpath "{classpath}" \
    {main_class} \
    --gameDir "$game" \
    --assetsDir "$assets" \
    --assetIndex {asset_index} \
    "$@"
