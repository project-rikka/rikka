#!{bash}
set -eu
if [ -z "${BUILD_WORKSPACE_DIRECTORY:-}" ]; then
    echo "{label}: start it with bazel run" >&2
    exit 1
fi
sources="$PWD/{sources}"
destination="$BUILD_WORKSPACE_DIRECTORY/{destination}"
rm -rf "$destination"
mkdir -p "$destination"
cd "$destination"
"{jar}" --extract --file "$sources"
echo "wrote Minecraft {version} sources to $destination"
